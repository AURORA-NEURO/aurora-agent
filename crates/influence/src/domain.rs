//! Blueprint 43.11: what an abstract domain is, and what 43.11 leaves to the implementer.
//!
//! 43.11 gives one thing precisely — the soundness condition
//!
//! ```text
//!     f(γ(a))  ⊆  γ(f#(a))
//! ```
//!
//! for a concrete operation `f`, its abstract counterpart `f#`, and a concretisation `γ` — and
//! names no concrete domain, no lattice, no widening operator and no refinement strategy. Every
//! concrete domain in [`crate::domains`] is therefore an invention of this implementation, and the
//! module that defines each one says so in its own words rather than presenting itself as spec.
//!
//! ## Why a domain is a trait with an associated element type
//!
//! Two of the three shipped domains have structurally identical lattices: both are intervals of
//! non-negative reals. [`crate::domains::RatioInterval`] abstracts a *pointwise reweighting of the
//! joint measure* and [`crate::domains::Displacement`] abstracts an *accumulated total-variation
//! budget*. Their join, meet and widening are the same arithmetic; their concretisations are
//! different sets of a different kind of object, and a bound derived by feeding one to the other's
//! transformer would be a number with no theorem behind it.
//!
//! Giving each domain its own `Element` type is what makes that mistake a compile error on the
//! static path: `RatioIntervalDomain::join` will not accept a `Displacement`. The dynamic path —
//! which exists because a compiler selects domains from a registry at run time, not at monomorphi-
//! sation time — cannot be checked by the type system, so [`crate::registry`] tags every value with
//! its [`DomainId`] and refuses a foreign one with a typed error. Neither path can mix domains
//! silently, which is the whole requirement.
//!
//! ## The obligations a domain takes on
//!
//! A domain that implements this trait is asserting, and [`laws`] is where a test can check:
//!
//! - `⊥ ⊑ a ⊑ ⊤` for every `a`, and `γ(⊥) = ∅`.
//! - `join` is a least *upper bound* in the ordering — in particular `γ(a) ∪ γ(b) ⊆ γ(a ⊔ b)`.
//!   A join that merely unions two representations without over-approximating their concretisations
//!   is not a join, and [`laws::join_over_approximates_concretisation`] is the check that fails.
//! - `meet` is a lower bound. It is used only to *descend* from a post-fixpoint, never to reach
//!   one, so it is allowed to be imprecise but not unsound.
//! - `widen` is an upper bound of both arguments — that is what keeps a widened iterate sound —
//!   *and* terminates every ascending chain. The second obligation cannot be checked by a law on
//!   two elements; each domain states its termination argument in its own docs and
//!   [`crate::solver`] converts a violation into a typed error rather than an infinite loop.
//!
//! ## What γ is here, and why it is a predicate
//!
//! `γ(a)` is generally infinite, so it is represented as a membership test,
//! [`AbstractDomain::concretises`], rather than as a set. The soundness condition is then checked
//! the way the rest of this crate checks things: pick concrete inputs, apply the concrete
//! operation, and assert the result is in `γ` of the abstract result. Where the concrete class is
//! finite the check is a proof; where it is not, it is a falsification search, and
//! [`EnumerableConcretisation::universe_is_complete`] is the flag that keeps those two apart. The
//! same distinction `crate::bruteforce` draws between removal and the multiplicative range.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// The name a domain is registered and referred to by.
///
/// A string rather than a type id: the registry is consulted by a compiler pass that reads a
/// domain name out of a plan, and a `TypeId` does not survive being written down.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DomainId(String);

impl DomainId {
    pub fn new(name: impl Into<String>) -> Self {
        DomainId(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DomainId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// The class of facts a domain is entitled to abstract.
///
/// 43.11 speaks of abstracting "query and evidence structure" without enumerating what an abstract
/// element stands for, so this enumeration is an invention. It exists so that a caller asking the
/// registry for a domain can ask for one that abstracts the right *kind* of thing, and so that a
/// registry holding two interval domains can report that they are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactClass {
    /// One non-negative real per joint assignment: the multiplicative reweighting a perturbation
    /// applies to the joint measure. See [`crate::ratio`] for the lemma that turns a bound on this
    /// class into a bound on an answer.
    JointReweighting,
    /// One non-negative real: an accumulated total-variation budget. Bounded by one when it is
    /// finally read as a distance, unbounded while it is being accumulated along paths.
    AnswerDisplacement,
    /// A factor's potential, entry by entry.
    FactorPotential,
}

impl FactClass {
    pub fn as_str(self) -> &'static str {
        match self {
            FactClass::JointReweighting => "joint_reweighting",
            FactClass::AnswerDisplacement => "answer_displacement",
            FactClass::FactorPotential => "factor_potential",
        }
    }
}

/// A lattice with a concretisation, in the sense of 43.11.
///
/// Implementors are in [`crate::domains`]. The trait is deliberately small: everything a solver
/// needs and nothing a solver does not, because every method here is an obligation somebody has to
/// discharge for each new domain.
pub trait AbstractDomain {
    /// The abstract element. Distinct per domain even when two domains share a shape, because that
    /// distinctness is what makes cross-domain misuse a compile error on the static path.
    type Element: Clone + PartialEq + fmt::Debug + 'static;

    /// The concrete object `γ` produces a set of.
    type Concrete: Clone + fmt::Debug + 'static;

    fn id(&self) -> DomainId;

    /// The class of facts this domain abstracts. Two domains with the same class are alternative
    /// abstractions of the same thing; two with different classes are not comparable at all.
    fn abstracts(&self) -> FactClass;

    /// The least element. `γ(⊥) = ∅`: nothing concretises to it, and reaching it means the analysis
    /// proved the situation unreachable rather than that it found the value zero.
    fn bottom(&self) -> Self::Element;

    /// The greatest element. `γ(⊤)` is everything the class contains, so an analysis that returns
    /// `⊤` has learned nothing — which is a result, and must not be dressed as a bound. See
    /// [`mod@crate::interpret`] for where that rule is enforced.
    fn top(&self) -> Self::Element;

    fn leq(&self, left: &Self::Element, right: &Self::Element) -> bool;

    fn join(&self, left: &Self::Element, right: &Self::Element) -> Self::Element;

    fn meet(&self, left: &Self::Element, right: &Self::Element) -> Self::Element;

    /// An upper bound of both arguments that also terminates every ascending chain.
    ///
    /// `previous` is the current iterate and `next` the one the transformer proposes. Returning
    /// `join(previous, next)` is a legal widening exactly when the lattice has no infinite
    /// ascending chains, which is true of [`crate::domains::SupportDomain`] and false of the two
    /// interval domains.
    fn widen(&self, previous: &Self::Element, next: &Self::Element) -> Self::Element;

    /// `concrete ∈ γ(element)`.
    fn concretises(&self, element: &Self::Element, concrete: &Self::Concrete) -> bool;

    /// A human-readable element, for the validity string a bound carries onto a certificate.
    fn render(&self, element: &Self::Element) -> String;
}

/// A domain whose concretisation can be checked against a finite set of concrete values.
///
/// Implemented where a soundness check can be a proof rather than a search. The flag is separate
/// from the universe because a finite *sample* of an infinite class is also useful — it is just not
/// a proof, and a suite that could not tell the two apart would report a falsification search as a
/// verification.
pub trait EnumerableConcretisation: AbstractDomain {
    fn concrete_universe(&self) -> Vec<Self::Concrete>;

    /// Whether every concrete value is represented in [`Self::concrete_universe`] up to the
    /// equivalence `γ` cannot distinguish.
    ///
    /// `true` licenses the word *proof* about a law checked over the universe. `false` means the
    /// universe is a grid through an infinite class and a passing check is evidence, not proof.
    fn universe_is_complete(&self) -> bool;
}

/// A misuse of the domain machinery, or a defect in a domain's own operators.
///
/// Distinct from [`crate::UnknownReason`] for the reason `crate::error` gives: this is a
/// well-formed question nobody asked correctly, not a well-formed question no method may answer.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum DomainError {
    #[error("domain {id} is already registered; silently replacing it would let one domain's transformer receive another domain's abstraction, which is the mistake the registry exists to prevent")]
    DuplicateRegistration { id: DomainId },

    #[error("no domain named {id} is registered")]
    UnregisteredDomain { id: DomainId },

    #[error("an abstract value from domain {found} was passed to an operation of domain {expected}; a lattice operation between two domains denotes nothing")]
    ForeignAbstractValue { expected: DomainId, found: DomainId },

    #[error("a value tagged {id} does not carry that domain's element type; the tag and the payload disagree")]
    ElementTypeMismatch { id: DomainId },

    #[error("domain {id} is registered but no transformer for it is implemented; a registry entry is an abstraction, and an abstraction with no transformer analyses nothing")]
    NoTransformerForDomain { id: DomainId },

    #[error("the ascending chain had not stabilised after {steps} joins; returning the last iterate would return a pre-fixpoint, which is not an over-approximation of anything")]
    AscendingChainDidNotStabilise { steps: usize },

    #[error("domain {id} widened {steps} times without reaching a post-fixpoint; a widening operator that does not terminate an ascending chain is not a widening operator")]
    WideningDidNotStabilise { id: DomainId, steps: usize },
}

/// The properties a domain claims, written as checks a test can run.
///
/// These are not invoked by the analysis. They exist because "join is a join" is the kind of claim
/// that is obviously true until a domain grows a third variant, and a law that only lives in a doc
/// comment is a law nobody re-checks.
pub mod laws {
    use super::{AbstractDomain, EnumerableConcretisation};

    /// `a ⊑ a ⊔ b` and `b ⊑ a ⊔ b`.
    pub fn join_is_an_upper_bound<D: AbstractDomain>(
        domain: &D,
        left: &D::Element,
        right: &D::Element,
    ) -> bool {
        let joined = domain.join(left, right);
        domain.leq(left, &joined) && domain.leq(right, &joined)
    }

    /// `a ⊔ b = b ⊔ a`.
    pub fn join_is_commutative<D: AbstractDomain>(
        domain: &D,
        left: &D::Element,
        right: &D::Element,
    ) -> bool {
        domain.join(left, right) == domain.join(right, left)
    }

    /// `a ⊔ a = a`.
    pub fn join_is_idempotent<D: AbstractDomain>(domain: &D, element: &D::Element) -> bool {
        domain.join(element, element) == *element
    }

    /// `(a ⊔ b) ⊔ c = a ⊔ (b ⊔ c)`.
    pub fn join_is_associative<D: AbstractDomain>(
        domain: &D,
        first: &D::Element,
        second: &D::Element,
        third: &D::Element,
    ) -> bool {
        let left = domain.join(&domain.join(first, second), third);
        let right = domain.join(first, &domain.join(second, third));
        left == right
    }

    /// `a ⊓ b ⊑ a` and `a ⊓ b ⊑ b`.
    pub fn meet_is_a_lower_bound<D: AbstractDomain>(
        domain: &D,
        left: &D::Element,
        right: &D::Element,
    ) -> bool {
        let met = domain.meet(left, right);
        domain.leq(&met, left) && domain.leq(&met, right)
    }

    /// `a ⊑ widen(a, b)` and `b ⊑ widen(a, b)`.
    ///
    /// The soundness half of the widening contract. The termination half is a property of chains,
    /// not of a pair, and each domain argues it in prose.
    pub fn widening_is_an_upper_bound<D: AbstractDomain>(
        domain: &D,
        previous: &D::Element,
        next: &D::Element,
    ) -> bool {
        let widened = domain.widen(previous, next);
        domain.leq(previous, &widened) && domain.leq(next, &widened)
    }

    /// `γ(a) ∪ γ(b) ⊆ γ(a ⊔ b)`, checked over the domain's concrete universe.
    ///
    /// This is the property that separates a join from a union of representations. A "join" that
    /// stored both operands side by side would satisfy the ordering laws above and fail this one
    /// the moment the analysis read a single bound off the result.
    pub fn join_over_approximates_concretisation<D: EnumerableConcretisation>(
        domain: &D,
        left: &D::Element,
        right: &D::Element,
    ) -> bool {
        let joined = domain.join(left, right);
        domain.concrete_universe().iter().all(|concrete| {
            let in_either =
                domain.concretises(left, concrete) || domain.concretises(right, concrete);
            !in_either || domain.concretises(&joined, concrete)
        })
    }

    /// `a ⊑ b` implies `γ(a) ⊆ γ(b)`, checked over the domain's concrete universe.
    pub fn order_implies_concretisation_inclusion<D: EnumerableConcretisation>(
        domain: &D,
        left: &D::Element,
        right: &D::Element,
    ) -> bool {
        if !domain.leq(left, right) {
            return true;
        }
        domain.concrete_universe().iter().all(|concrete| {
            !domain.concretises(left, concrete) || domain.concretises(right, concrete)
        })
    }

    /// `γ(⊥) = ∅`, checked over the domain's concrete universe.
    pub fn bottom_concretises_to_nothing<D: EnumerableConcretisation>(domain: &D) -> bool {
        let bottom = domain.bottom();
        domain
            .concrete_universe()
            .iter()
            .all(|concrete| !domain.concretises(&bottom, concrete))
    }

    /// `γ(⊤)` contains everything in the universe.
    pub fn top_concretises_to_everything<D: EnumerableConcretisation>(domain: &D) -> bool {
        let top = domain.top();
        domain
            .concrete_universe()
            .iter()
            .all(|concrete| domain.concretises(&top, concrete))
    }
}
