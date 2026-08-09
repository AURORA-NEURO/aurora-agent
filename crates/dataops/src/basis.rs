//! How an answer came to be known, and over how much of the thing it was asked about.
//!
//! Every module in this crate produces answers that did not come from the authoritative source.
//! 12.02 serves search results from an index rebuilt out of the catalog. 12.03 lets a projection
//! run behind an outbox. 12.12 computes an availability figure from telemetry that may have gaps.
//! 12.13 reads a provider's own published conformance claim. 12.14 places a job against a
//! worker's self-declared capabilities. 12.16 answers from a replicated catalog belonging to
//! another hub. In each case the value is the same shape as an authoritative one, and in each
//! case treating it as authoritative is the failure.
//!
//! # Why a sixth freshness type, and how this one differs
//!
//! Three siblings already refuse to let a stale answer look fresh, and each does it on a
//! different axis. `bioprism-infra`'s index reports *lag*: how many epochs a projection is behind
//! its source, with a third state for "the caller did not say". `bioprism-hubapi`'s mirror
//! reports *replication*: whether an answer came from an origin or a copy, with no `is_current`
//! at all. `bioprism-tokens` reports *validity*: whether a compiled context is inside a declared
//! TTL, on a biological axis kept separate from a cache axis.
//!
//! None of the three answers the question this crate keeps hitting, which is not *how old is it*
//! but ***who looked, and at how much of it***. A provider that publishes `conformance: full` and
//! a conformance suite the platform ran itself produce the identical value. An availability of
//! 99.9% computed from every request in the window and one computed from the four samples that
//! survived a telemetry outage produce the identical value. [`Basis`] separates the first pair;
//! [`Coverage`] separates the second.
//!
//! # No `is_fresh`, no `is_trustworthy`
//!
//! Following `bioprism-hubapi` and `bioprism-tokens`, there is no predicate on [`Basis`] that
//! collapses it to a boolean, because every such predicate is a place where five states become
//! two and the caller stops seeing the difference. What exists instead is
//! [`Basis::is_first_hand`], whose name says exactly what it tests, and
//! [`Basis::observed_at`], which returns `None` for the one variant that has no observation
//! behind it.
//!
//! # The structural half
//!
//! [`Attested<T>`] carries a value with its basis and its coverage, has private fields, and has
//! no `From<T>` and no `Deref`. There is no way to obtain one without stating how the value was
//! known. Its `PartialEq` compares all three components, so **two attested values that agree on
//! the value and disagree on the basis are not equal** — the invariant this section keeps needing,
//! expressed as the one operator every caller already reaches for.
//!
//! # Not implemented
//!
//! No signatures and no attestation verification. A [`Basis::Declared`] records *that* a party
//! declared something, never that a signature over it checked out; 12.13's "publishes" and
//! 12.14's "remote attestation where justified" both need cryptography and a key distribution
//! story this crate does not have. [`PartyId`] is a name, not a principal.

use crate::error::BasisError;
use bioprism_infra::Epoch;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Something that can assert a fact about itself: a worker, a provider, a federated hub.
///
/// Deliberately a bare name. Turning this into a verified principal requires signatures, which
/// this crate does not have, and a `PartyId` that looked like a principal would invite callers to
/// treat [`Basis::Declared`] as evidence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PartyId(String);

impl PartyId {
    pub fn parse(value: impl Into<String>) -> Result<Self, BasisError> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(BasisError::MalformedField {
                field: "party id",
                value,
            });
        }
        Ok(PartyId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PartyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for PartyId {
    type Error = BasisError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        PartyId::parse(value)
    }
}

impl From<PartyId> for String {
    fn from(value: PartyId) -> Self {
        value.0
    }
}

/// How a value came to be known.
///
/// The ordering of the variants is deliberate and is the ordering [`Basis::weakest_of`] uses: a
/// composite answer is exactly as good as the worst thing it rested on. It is *not* a quality
/// score — nothing here multiplies or averages bases — it is a lattice with one join.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum Basis {
    /// The platform measured it, here, at this epoch.
    FirstHand { observed_at: Epoch },
    /// Copied from an origin's answer as that answer stood at `origin_epoch`.
    ///
    /// Two epochs because they answer different questions: `origin_epoch` bounds what the copy
    /// can possibly contain, `received_at` bounds how long the local side has had it. A federated
    /// import that recorded only one of them cannot distinguish a fast copy of stale data from a
    /// slow copy of fresh data.
    Replicated {
        origin: PartyId,
        origin_epoch: Epoch,
        received_at: Epoch,
    },
    /// Recomputed from a projection that lags its canonical source by `lag_epochs`.
    Derived { source: String, lag_epochs: u64 },
    /// The subject said so. Nobody checked.
    Declared { by: PartyId, declared_at: Epoch },
    /// Nobody looked.
    ///
    /// Carries a reason so that this can never be produced as a silent default: a caller has to
    /// write down why the observation is missing, which is the moment they notice it is.
    Unobserved { reason: String },
}

impl Basis {
    /// True only for [`Basis::FirstHand`].
    ///
    /// Named for what it tests rather than for a judgement about it. First-hand is not the same
    /// as correct, and a replicated answer inside its bound may be perfectly usable; that call
    /// belongs to a policy, and every policy in this crate is a value the caller supplies.
    pub fn is_first_hand(&self) -> bool {
        matches!(self, Basis::FirstHand { .. })
    }

    /// The epoch at which the observation behind this value was made, if there was one.
    ///
    /// `None` for [`Basis::Unobserved`] and for [`Basis::Derived`], which knows its lag but not
    /// the epoch of the underlying source. Returning an `Option` rather than `Epoch::ZERO` keeps
    /// "no observation" from arriving as "an observation at the beginning of time".
    pub fn observed_at(&self) -> Option<Epoch> {
        match self {
            Basis::FirstHand { observed_at } => Some(*observed_at),
            Basis::Replicated { origin_epoch, .. } => Some(*origin_epoch),
            Basis::Declared { declared_at, .. } => Some(*declared_at),
            Basis::Derived { .. } | Basis::Unobserved { .. } => None,
        }
    }

    /// Epochs between the underlying observation and `now`, or `None` when there is no
    /// observation to measure from or `now` precedes it.
    pub fn age_at(&self, now: Epoch) -> Option<u64> {
        now.elapsed_since(self.observed_at()?)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Basis::FirstHand { .. } => "first-hand",
            Basis::Replicated { .. } => "replicated",
            Basis::Derived { .. } => "derived",
            Basis::Declared { .. } => "declared",
            Basis::Unobserved { .. } => "unobserved",
        }
    }

    /// Rank in the lattice: 0 is first-hand, 4 is unobserved.
    fn rank(&self) -> u8 {
        match self {
            Basis::FirstHand { .. } => 0,
            Basis::Replicated { .. } => 1,
            Basis::Derived { .. } => 2,
            Basis::Declared { .. } => 3,
            Basis::Unobserved { .. } => 4,
        }
    }

    /// The weakest basis among several, or [`Basis::Unobserved`] when there are none.
    ///
    /// A decision that consulted a verified fact and a declared one rests on the declared one.
    /// An empty input is not a strong answer: a placement that matched zero facts observed
    /// nothing, and saying so is the whole point of this function existing rather than callers
    /// writing `bases.first().cloned().unwrap_or(FirstHand)`.
    pub fn weakest_of<'a>(bases: impl IntoIterator<Item = &'a Basis>) -> Basis {
        bases
            .into_iter()
            .max_by_key(|basis| basis.rank())
            .cloned()
            .unwrap_or(Basis::Unobserved {
                reason: "no facts were consulted".to_string(),
            })
    }
}

impl fmt::Display for Basis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How much of the intended population was actually observed.
///
/// The third variant is the one this type exists for. 12.12 lists eight service objectives —
/// "API read availability", "operation acceptance", "queue-start latency by priority" — and gives
/// none of them a denominator, so an implementation that counts the requests it happened to
/// receive telemetry for will report a rate. [`Coverage::NoDenominator`] is what an honest
/// implementation reports instead: observations exist, the population they were drawn from is
/// unknown, and therefore no rate exists.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "coverage", rename_all = "snake_case")]
pub enum Coverage {
    /// Every member of a known population was observed.
    Complete { observed: u64 },
    /// `observed` of a known `expected` were seen.
    Partial { observed: u64, expected: u64 },
    /// Observations were made; how many there should have been is unknown.
    NoDenominator { observed: u64, reason: String },
}

impl Coverage {
    /// Classifies by comparing an observed count against an expected one.
    ///
    /// Rejects `observed > expected` rather than clamping. More observations than the population
    /// admits means the population figure is wrong, and a silently clamped `Complete` would erase
    /// exactly that signal.
    pub fn of(observed: u64, expected: u64) -> Result<Coverage, BasisError> {
        if observed > expected {
            return Err(BasisError::CoverageExceedsPopulation { observed, expected });
        }
        if observed == expected {
            Ok(Coverage::Complete { observed })
        } else {
            Ok(Coverage::Partial { observed, expected })
        }
    }

    pub fn observed(&self) -> u64 {
        match self {
            Coverage::Complete { observed }
            | Coverage::Partial { observed, .. }
            | Coverage::NoDenominator { observed, .. } => *observed,
        }
    }

    /// The population the observations were drawn from, or `None` when it is unknown.
    ///
    /// This is the function that makes a rate expressible. Everything downstream that wants to
    /// divide has to call it and handle the `None`.
    pub fn expected(&self) -> Option<u64> {
        match self {
            Coverage::Complete { observed } => Some(*observed),
            Coverage::Partial { expected, .. } => Some(*expected),
            Coverage::NoDenominator { .. } => None,
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Coverage::Complete { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Coverage::Complete { .. } => "complete",
            Coverage::Partial { .. } => "partial",
            Coverage::NoDenominator { .. } => "no-denominator",
        }
    }

    /// The weakest coverage among several: complete only if all are complete, and unknown as soon
    /// as one denominator is unknown.
    pub fn weakest_of<'a>(coverages: impl IntoIterator<Item = &'a Coverage>) -> Coverage {
        let mut observed = 0u64;
        let mut expected = Some(0u64);
        let mut missing: Option<String> = None;
        for coverage in coverages {
            observed = observed.saturating_add(coverage.observed());
            match coverage.expected() {
                Some(value) => expected = expected.map(|total| total.saturating_add(value)),
                None => {
                    expected = None;
                    if missing.is_none() {
                        missing = Some(coverage.name().to_string());
                    }
                }
            }
        }
        match expected {
            Some(expected) if expected == observed => Coverage::Complete { observed },
            Some(expected) => Coverage::Partial { observed, expected },
            None => Coverage::NoDenominator {
                observed,
                reason: "at least one component had no known population".to_string(),
            },
        }
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A value that cannot be read without its basis and coverage.
///
/// Fields are private and there is no `From<T>`, no `Deref` and no `into_inner` that drops the
/// provenance. [`Attested::value`] hands out a reference while the wrapper stays in the caller's
/// hand, so the two travel together through every structure in this crate.
///
/// `PartialEq` covers all three fields. That is the load-bearing derive: a conformance level of
/// `Full` that the platform verified and a conformance level of `Full` that a provider asserted
/// are `!=`, and any caller that compares them — a test, a cache key, a diff between two
/// deployment plans — sees the difference without having been told to look for it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Attested<T> {
    value: T,
    basis: Basis,
    coverage: Coverage,
}

impl<T> Attested<T> {
    /// The general constructor. Both provenance components are required arguments; there is no
    /// default for either.
    pub fn new(value: T, basis: Basis, coverage: Coverage) -> Self {
        Attested {
            value,
            basis,
            coverage,
        }
    }

    /// A single value the platform measured itself.
    ///
    /// Coverage is `Complete { observed: 1 }` because a single measured value is a complete
    /// observation of a population of one. This is the only place in the crate where a complete
    /// coverage is minted without a caller-supplied denominator, and it is sound precisely
    /// because the denominator is one.
    pub fn first_hand(value: T, observed_at: Epoch) -> Self {
        Attested {
            value,
            basis: Basis::FirstHand { observed_at },
            coverage: Coverage::Complete { observed: 1 },
        }
    }

    /// A value its subject asserted about itself.
    pub fn declared(value: T, by: PartyId, declared_at: Epoch) -> Self {
        Attested {
            value,
            basis: Basis::Declared { by, declared_at },
            coverage: Coverage::Complete { observed: 1 },
        }
    }

    pub fn value(&self) -> &T {
        &self.value
    }

    pub fn basis(&self) -> &Basis {
        &self.basis
    }

    pub fn coverage(&self) -> &Coverage {
        &self.coverage
    }

    /// Applies a function to the value, keeping the provenance attached.
    ///
    /// Deliberately cannot strengthen the basis: `f` sees only the value, so no projection of an
    /// attested value can turn a declaration into a measurement.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Attested<U> {
        Attested {
            value: f(self.value),
            basis: self.basis,
            coverage: self.coverage,
        }
    }
}
