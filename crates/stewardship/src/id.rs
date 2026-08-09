//! The small vocabulary every module here shares: actors, roles and operator epochs.
//!
//! # There is no identity system
//!
//! An [`ActorId`] is an opaque string, exactly as `bioprism_hub::SubmitterId` is. Nothing in this
//! crate authenticates it, and no function here can tell a real reviewer from a name typed into a
//! JSON document. What the crate does with an actor is *structural*: it compares one against
//! another to enforce separation of duties, and it records which one made a decision so that the
//! decision has an author. Both of those are checks on a document, not on a person.
//!
//! This matters because section 14's governance modules are, almost entirely, statements about
//! people — councils, votes, recusals, disclosures, appeals. A type named `Council` with a `vote`
//! method would assert that a council met, which is precisely the thing software cannot know. The
//! honest reduction is: names are strings, and the only questions asked about them are questions a
//! reader of the record could ask too.
//!
//! # There is no clock
//!
//! [`Epoch`] is an operator-supplied monotone counter, following `bioprism_governance`'s
//! deprecation lifecycle and `bioprism_ledger`. Nothing here reads the system time. A grant expires
//! because an epoch was advanced past its ceiling, not because a wall clock passed a date, so every
//! lifecycle in this crate replays identically.

use serde::{Deserialize, Serialize};
use std::fmt;

/// An opaque name for a party that can review, decide, grant, or claim.
///
/// Deliberately not a key, a certificate, or an account. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActorId(String);

impl ActorId {
    pub fn new(name: impl Into<String>) -> Self {
        ActorId(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Monotone operator-supplied ordinal. Not a timestamp; see the module docs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(transparent)]
pub struct Epoch(pub u64);

impl Epoch {
    pub const fn next(self) -> Epoch {
        Epoch(self.0 + 1)
    }
}

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "epoch {}", self.0)
    }
}

/// The standing a party holds when it acts.
///
/// Section 14's module 03 lists ten roles; this is not that list, and does not try to be. These are
/// the four distinctions the *checks* in this crate actually make: whether a party authored the
/// thing under review, whether it is independent of the author, whether it is a domain expert whose
/// sign-off the medical boundary requires, and whether it stewards data. A role enumeration that
/// mirrored a maintainer model would be modelling an organisation nobody here can observe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Built or owns the artifact under review.
    Author,
    /// Reviews on behalf of the platform, with no stake in the artifact.
    IndependentReviewer,
    /// A clinician or domain scientist whose sign-off the medical boundary requires.
    DomainExpert,
    /// Holds a data contract and decides access under it.
    DataSteward,
}

impl Role {
    /// Whether a party in this role may be the sole approver of somebody else's work.
    ///
    /// [`Role::Author`] cannot, which is the one rule section 14 states in four different modules:
    /// "no benchmark author unilaterally validates their own primary leaderboard claim",
    /// "author cannot provide the only approval for scoring semantics", "authors do not solely
    /// certify their own systems", "conflicted person may provide factual input but cannot be sole
    /// reviewer".
    pub const fn may_approve_alone(self) -> bool {
        !matches!(self, Role::Author)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Role::Author => "author",
            Role::IndependentReviewer => "independent_reviewer",
            Role::DomainExpert => "domain_expert",
            Role::DataSteward => "data_steward",
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A party acting in a role, which is the unit every decision in this crate is attributed to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Actor {
    pub id: ActorId,
    pub role: Role,
}

impl Actor {
    pub fn new(id: impl Into<String>, role: Role) -> Self {
        Actor {
            id: ActorId::new(id),
            role,
        }
    }

    pub fn independent_reviewer(id: impl Into<String>) -> Self {
        Actor::new(id, Role::IndependentReviewer)
    }

    pub fn author(id: impl Into<String>) -> Self {
        Actor::new(id, Role::Author)
    }

    pub fn domain_expert(id: impl Into<String>) -> Self {
        Actor::new(id, Role::DomainExpert)
    }

    pub fn steward(id: impl Into<String>) -> Self {
        Actor::new(id, Role::DataSteward)
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.id, self.role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_author_may_never_be_the_sole_approver_of_their_own_work() {
        assert!(!Role::Author.may_approve_alone());
        assert!(Role::IndependentReviewer.may_approve_alone());
        assert!(Role::DomainExpert.may_approve_alone());
    }

    #[test]
    fn an_epoch_orders_but_does_not_date() {
        assert!(Epoch(1) < Epoch(2));
        assert_eq!(Epoch(1).next(), Epoch(2));
        assert_eq!(serde_json::to_string(&Epoch(7)).unwrap(), "7");
    }

    #[test]
    fn an_actor_round_trips_through_json_unchanged() {
        let actor = Actor::domain_expert("neuro-reviewer-2");
        let text = serde_json::to_string(&actor).unwrap();
        assert_eq!(serde_json::from_str::<Actor>(&text).unwrap(), actor);
    }
}
