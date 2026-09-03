//! How well something can be put back, said out loud.
//!
//! Three blueprint modules in this crate's scope ask for the same thing in three vocabularies.
//! 03.03 (World State IR) says each component "declares capture method and restoration
//! confidence". 04.04 (Environment and Artifact Capture) says "unpinned dependencies reduce
//! reproducibility level". 05.11 (Shepherd and External Fork-Runtime Integration) says that when
//! the external runtime is unavailable, PRISM "reconstructs a world state in the local executor and
//! *lowers the reproducibility declaration*".
//!
//! Reduce, lower, declare. That is a meet-semilattice with a monotone-downward update, and this
//! module is it.
//!
//! # Why four levels and not three
//!
//! The obvious encoding is exact / lossy / missing. It loses the distinction this workspace keeps
//! re-deriving. 03.03's equality rules end with "declared equivalence for nondeterministic
//! services": a component that will *never* restore byte-identically and about which somebody has
//! nonetheless established that the difference cannot change the decision. That is not the same
//! state as a component that differs and nobody worked out whether it matters. So:
//!
//! | Level | Meaning |
//! |---|---|
//! | [`Level::Exact`] | captured, restores identically |
//! | [`Level::Equivalent`] | captured, does not restore identically, and a named basis says the difference cannot matter |
//! | [`Level::Degraded`] | captured, does not restore identically, and nobody has shown the difference cannot matter |
//! | [`Level::Absent`] | not captured, and said so |
//!
//! There is deliberately **no fifth level for "not captured and not mentioned"**. That state is
//! unrepresentable rather than merely discouraged: [`Declaration`] has no `Default`, every
//! constructor takes a level, and the constructors below `Exact` require a non-empty basis. A
//! caller that forgot about a component cannot produce a `Declaration` for it at all — which is
//! why the callers in [`crate::capture`] and [`crate::state`] enumerate their dimensions and error
//! on the ones with no entry, rather than filling a gap with a benign-looking value.
//!
//! # What this is not
//!
//! It is not a probability and not a score. `Equivalent` above `Degraded` is an ordering of
//! epistemic states, not a measurement of how much was lost, and the blueprint supplies no numbers
//! for the latter. Nothing here multiplies or averages.

use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};

/// The four restoration states, ordered worst to best.
///
/// The `Ord` derive is load-bearing: [`meet`] is `min`, and the variant order below *is* the
/// semantics. Reordering the variants changes the lattice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// Not captured. The declaration says so and says why.
    Absent,
    /// Captured, restores differently, and no basis says the difference is immaterial.
    Degraded,
    /// Captured, restores differently, and a named basis says the difference cannot matter.
    ///
    /// 03.03's "declared equivalence for nondeterministic services".
    Equivalent,
    /// Captured, restores identically.
    Exact,
}

impl Level {
    /// A stable name for diagnostics. Used by [`SweepError::FidelityRaised`], which needs a
    /// `&'static str` rather than a `Display` impl allocating a `String` inside an error path.
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Absent => "absent",
            Level::Degraded => "degraded",
            Level::Equivalent => "equivalent",
            Level::Exact => "exact",
        }
    }

    /// Whether a difference at this level is known not to affect a decision.
    ///
    /// True for `Exact` and `Equivalent` only. `Degraded` is not "a bit worse than equivalent";
    /// it is the state where the question was not answered.
    pub fn difference_is_immaterial(self) -> bool {
        matches!(self, Level::Exact | Level::Equivalent)
    }
}

/// A level together with the basis for it.
///
/// Anything below [`Level::Exact`] carries a non-empty basis, because 03.03 asks for a declared
/// restoration confidence and a bare enum variant declares nothing a reviewer can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    level: Level,
    basis: String,
}

impl Declaration {
    /// The component restores identically. No basis is required because there is no difference to
    /// explain.
    pub fn exact() -> Self {
        Declaration {
            level: Level::Exact,
            basis: String::new(),
        }
    }

    /// The component restores differently and `basis` says why that cannot change the decision.
    ///
    /// The basis is the whole content of the claim. "Nondeterministic service" is not a basis;
    /// "response tape replays the same bodies, only the `Date` header differs" is.
    pub fn equivalent(basis: impl Into<String>) -> Result<Self, SweepError> {
        let basis = basis.into();
        require_nonempty(&basis, "Declaration::equivalent", "basis")?;
        Ok(Declaration {
            level: Level::Equivalent,
            basis,
        })
    }

    /// The component restores differently and nobody has established that it does not matter.
    pub fn degraded(reason: impl Into<String>) -> Result<Self, SweepError> {
        let reason = reason.into();
        require_nonempty(&reason, "Declaration::degraded", "reason")?;
        Ok(Declaration {
            level: Level::Degraded,
            basis: reason,
        })
    }

    /// The component was not captured, and this says so.
    pub fn absent(reason: impl Into<String>) -> Result<Self, SweepError> {
        let reason = reason.into();
        require_nonempty(&reason, "Declaration::absent", "reason")?;
        Ok(Declaration {
            level: Level::Absent,
            basis: reason,
        })
    }

    pub fn level(&self) -> Level {
        self.level
    }

    /// Empty exactly when the level is [`Level::Exact`].
    pub fn basis(&self) -> &str {
        &self.basis
    }

    /// Lower this declaration, recording why.
    ///
    /// The only update operation. 05.11's fallback and 04.04's unpinned dependency both call it,
    /// and both would be bugs if they could call the inverse — a run that fell back to local
    /// reconstruction and then reported `Exact` is precisely the silent degradation the blueprint's
    /// failure list names.
    pub fn lowered_to(&self, level: Level, reason: impl Into<String>) -> Result<Self, SweepError> {
        if level > self.level {
            return Err(SweepError::FidelityRaised {
                what: "restoration declaration",
                from: self.level.as_str(),
                to: level.as_str(),
            });
        }
        if level == self.level {
            return Ok(self.clone());
        }
        match level {
            Level::Exact => Err(SweepError::malformed(
                "Declaration::lowered_to",
                "an exact declaration cannot be a strict lower level",
            )),
            Level::Equivalent => Declaration::equivalent(reason),
            Level::Degraded => Declaration::degraded(reason),
            Level::Absent => Declaration::absent(reason),
        }
    }
}

/// The worse of two declarations, keeping the basis of the side that lost.
///
/// Ties keep the left. That is arbitrary but deterministic, and determinism is the property that
/// matters: a manifest's overall fidelity must not depend on the iteration order of a map.
pub fn meet(left: &Declaration, right: &Declaration) -> Declaration {
    if right.level < left.level {
        right.clone()
    } else {
        left.clone()
    }
}

/// The meet over a sequence, or [`Declaration::exact`] over an empty one.
///
/// The empty case is the lattice's top element, which is correct arithmetically and dangerous
/// semantically: an empty component list means "nothing was captured", not "everything restored
/// perfectly". Callers must not hand this function an accidentally-empty collection, and
/// [`crate::state::WorldStateManifest`] refuses to be built with zero components for that reason.
pub fn meet_all<'a, I>(declarations: I) -> Declaration
where
    I: IntoIterator<Item = &'a Declaration>,
{
    declarations
        .into_iter()
        .fold(Declaration::exact(), |acc, next| meet(&acc, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eq(basis: &str) -> Declaration {
        Declaration::equivalent(basis).unwrap()
    }

    fn deg(reason: &str) -> Declaration {
        Declaration::degraded(reason).unwrap()
    }

    fn abs(reason: &str) -> Declaration {
        Declaration::absent(reason).unwrap()
    }

    #[test]
    fn declared_equivalence_outranks_unexplained_difference() {
        assert!(Level::Equivalent > Level::Degraded);
        assert!(eq("tape replays identical bodies")
            .level()
            .difference_is_immaterial());
        assert!(!deg("clock skew unmodelled")
            .level()
            .difference_is_immaterial());
    }

    #[test]
    fn a_declaration_below_exact_cannot_have_an_empty_basis() {
        assert!(matches!(
            Declaration::equivalent("  "),
            Err(SweepError::EmptyJustification { field: "basis", .. })
        ));
        assert!(Declaration::degraded("").is_err());
        assert!(Declaration::absent("").is_err());
    }

    #[test]
    fn meet_is_commutative_associative_and_idempotent() {
        let a = Declaration::exact();
        let b = eq("same bodies");
        let c = deg("process memory not captured");
        assert_eq!(meet(&a, &b).level(), meet(&b, &a).level());
        assert_eq!(
            meet(&meet(&a, &b), &c).level(),
            meet(&a, &meet(&b, &c)).level()
        );
        assert_eq!(meet(&c, &c), c);
    }

    #[test]
    fn meet_never_exceeds_either_input() {
        for left in [Declaration::exact(), eq("x"), deg("y"), abs("z")] {
            for right in [Declaration::exact(), eq("x"), deg("y"), abs("z")] {
                let m = meet(&left, &right);
                assert!(m.level() <= left.level());
                assert!(m.level() <= right.level());
            }
        }
    }

    #[test]
    fn meet_keeps_the_basis_of_the_losing_side() {
        let m = meet(&Declaration::exact(), &deg("process memory not captured"));
        assert_eq!(m.basis(), "process memory not captured");
    }

    #[test]
    fn a_declaration_cannot_be_raised() {
        let fallback = deg("external fork runtime unavailable");
        let err = fallback.lowered_to(Level::Exact, "wishful").unwrap_err();
        assert!(matches!(
            err,
            SweepError::FidelityRaised {
                from: "degraded",
                to: "exact",
                ..
            }
        ));
        assert!(fallback
            .lowered_to(Level::Equivalent, "still wishful")
            .is_err());
    }

    #[test]
    fn lowering_to_the_same_level_is_a_no_op_that_keeps_the_original_basis() {
        let d = deg("original reason");
        let same = d.lowered_to(Level::Degraded, "replacement reason").unwrap();
        assert_eq!(same.basis(), "original reason");
    }

    #[test]
    fn lowering_requires_a_reason_at_the_new_level() {
        let d = Declaration::exact();
        assert!(d.lowered_to(Level::Degraded, "").is_err());
    }

    #[test]
    fn meet_all_of_an_empty_sequence_is_exact_which_is_why_manifests_forbid_emptiness() {
        assert_eq!(meet_all(std::iter::empty()).level(), Level::Exact);
    }

    #[test]
    fn a_declaration_survives_a_json_round_trip_with_its_basis() {
        let d = eq("only the Date header differs");
        let json = serde_json::to_string(&d).unwrap();
        let back: Declaration = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
        assert_eq!(back.basis(), "only the Date header differs");
    }
}
