//! Mirrors, staleness and what "offline" is allowed to mean.
//!
//! Blueprint 10.04 gives offline one paragraph — *"Export a closure bundle containing manifests,
//! artifacts, images or references, signatures, keys, and revocation snapshot"* — and 10.18 adds
//! "offline export bundles support air-gapped environments". Both treat offline as an export
//! format. Neither says what a *resolution* performed against such a bundle is entitled to claim,
//! which is the question that actually matters, because this workspace is built offline against
//! pinned dependencies and a mirror is therefore the ordinary case rather than the fallback.
//!
//! # The rule
//!
//! A resolution against a mirror and a resolution against the origin must be **indistinguishable
//! in result and distinguishable in provenance**. Indistinguishable in result: the name, version
//! and digest are the same values, or the mirror is not a mirror and
//! [`MirrorError::Divergent`] says so. Distinguishable in provenance: the answer always carries
//! which registry produced it, whether that registry was the authority for the name, and how stale
//! its copy may be.
//!
//! # Staleness is declared, and a declaration is not a measurement
//!
//! A mirror states a [`StalenessBound`] — "my copy is never more than N epochs behind" — and the
//! epoch it last synchronised at. That is a promise the mirror makes about itself. Nothing here
//! verifies it, and nothing could: verifying it means asking the origin, which is the thing that
//! could not be reached. What [`Freshness`] does is keep the promise and the evidence apart, so a
//! consumer reading an answer can see it was a promise.
//!
//! # Undetermined is a first-class answer
//!
//! Deciding whether a mirror is within its bound requires a reference epoch — some notion of
//! "now". Air-gapped, there frequently is none, and the honest result is
//! [`Freshness::Undetermined`], not an optimistic default. This is the specific failure the module
//! exists to prevent: *"I could not reach the origin"* rendering as *"this is the current
//! version"*. Accordingly there is no `is_current` on [`Freshness`]. There is
//! [`Freshness::is_from_authority`], which is true only for an answer from the origin itself, and
//! [`Freshness::is_within_declared_bound`], whose name says whose claim it reports.
//!
//! A deployment that wants to proceed on an undetermined answer says so in
//! [`FreshnessPolicy::accept_undetermined`], and that acceptance travels with the resolution. The
//! difference between a deployment that decided to trust its bundle and one that never noticed is
//! then legible in the record.
//!
//! # Not implemented
//!
//! No replication, no transfer, no cursors, no bundle format. 10.18's "event cursors, digest
//! verification, tombstone propagation, resumable artifact transfer" are transport. Nothing here
//! copies anything anywhere; a [`Replication`] is a value describing a copy somebody else made.

use crate::registry::RegistryId;
use bioprism_hub::Epoch;
use serde::{Deserialize, Serialize};
use std::fmt;

/// How far behind a mirror promises never to be, in operator epochs.
///
/// Epochs rather than seconds, for the reason `bioprism-governance` gives: there is no clock in
/// this workspace, and a staleness bound that moved with the machine's date would make the same
/// bundle fresh on one host and stale on another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StalenessBound {
    pub max_lag_epochs: u64,
}

impl StalenessBound {
    /// A mirror claiming to be exactly current. Legal, and worth reading with suspicion: it is the
    /// strongest promise a mirror can make and the one it is least able to keep.
    pub const EXACT: StalenessBound = StalenessBound { max_lag_epochs: 0 };

    pub fn epochs(max_lag_epochs: u64) -> Self {
        StalenessBound { max_lag_epochs }
    }
}

impl fmt::Display for StalenessBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at most {} epoch(s) behind", self.max_lag_epochs)
    }
}

/// Where a catalog's contents came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sync", rename_all = "snake_case")]
pub enum Replication {
    /// The catalog is the origin's own. There is nothing to be behind.
    Origin,
    /// The catalog is a copy, taken at `synced_at`, promising to stay within `bound`.
    Mirror {
        of: RegistryId,
        synced_at: Epoch,
        bound: StalenessBound,
    },
}

impl Replication {
    pub fn mirror(of: RegistryId, synced_at: Epoch, bound: StalenessBound) -> Self {
        Replication::Mirror {
            of,
            synced_at,
            bound,
        }
    }

    pub fn is_origin(&self) -> bool {
        matches!(self, Replication::Origin)
    }

    /// Judges the copy against a reference epoch, or reports that there was none to judge against.
    ///
    /// `as_of` is `Option` rather than defaulted because the offline case is the one where it is
    /// genuinely absent, and a default would be a fabricated observation.
    pub fn freshness(&self, as_of: Option<Epoch>) -> Freshness {
        match self {
            Replication::Origin => Freshness::Authoritative,
            Replication::Mirror {
                synced_at, bound, ..
            } => {
                let Some(now) = as_of else {
                    return Freshness::Undetermined {
                        synced_at: *synced_at,
                        bound: *bound,
                    };
                };
                let lag = now.get().saturating_sub(synced_at.get());
                if now < *synced_at {
                    return Freshness::AheadOfReference {
                        synced_at: *synced_at,
                        reference: now,
                    };
                }
                if lag <= bound.max_lag_epochs {
                    Freshness::WithinBound {
                        lag,
                        bound: *bound,
                        synced_at: *synced_at,
                    }
                } else {
                    Freshness::BeyondBound {
                        lag,
                        bound: *bound,
                        synced_at: *synced_at,
                    }
                }
            }
        }
    }
}

/// How much an answer's currency is actually known.
///
/// Four outcomes, not two, and no boolean anywhere that collapses them. In particular a resolution
/// against a stale mirror carries [`Freshness::BeyondBound`] and one against a fresh mirror carries
/// [`Freshness::WithinBound`]; they are different values, and every consumer that pattern-matches
/// on freshness is forced to decide what it thinks about each.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "freshness", rename_all = "snake_case")]
pub enum Freshness {
    /// The origin answered. Currency is not in question because there is nothing to lag behind.
    Authoritative,
    /// A mirror answered and its copy is within the bound it declared.
    WithinBound {
        lag: u64,
        bound: StalenessBound,
        synced_at: Epoch,
    },
    /// A mirror answered and its copy is outside the bound it declared. The answer may still be
    /// correct — most stale mirrors are — but nothing here establishes that.
    BeyondBound {
        lag: u64,
        bound: StalenessBound,
        synced_at: Epoch,
    },
    /// A mirror answered and no reference epoch was supplied, so the bound could not be checked.
    /// The ordinary air-gapped outcome.
    Undetermined {
        synced_at: Epoch,
        bound: StalenessBound,
    },
    /// A mirror claims to have synchronised after the reference epoch. Somebody's epochs disagree,
    /// and reporting that as "fresh" would launder a bookkeeping fault into a currency guarantee.
    AheadOfReference { synced_at: Epoch, reference: Epoch },
}

impl Freshness {
    /// True only when the origin answered.
    pub fn is_from_authority(&self) -> bool {
        matches!(self, Freshness::Authoritative)
    }

    /// True when the answering registry's *own* declared bound is met. Named for whose claim it
    /// reports, because that is the whole content of it.
    pub fn is_within_declared_bound(&self) -> bool {
        matches!(
            self,
            Freshness::Authoritative | Freshness::WithinBound { .. }
        )
    }

    /// True when nothing at all is known about currency.
    pub fn is_undetermined(&self) -> bool {
        matches!(
            self,
            Freshness::Undetermined { .. } | Freshness::AheadOfReference { .. }
        )
    }

    pub fn lag(&self) -> Option<u64> {
        match self {
            Freshness::WithinBound { lag, .. } | Freshness::BeyondBound { lag, .. } => Some(*lag),
            _ => None,
        }
    }
}

impl fmt::Display for Freshness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Freshness::Authoritative => f.write_str("from the authority"),
            Freshness::WithinBound { lag, bound, .. } => {
                write!(f, "mirror {lag} epoch(s) behind, {bound}")
            }
            Freshness::BeyondBound { lag, bound, .. } => {
                write!(f, "mirror {lag} epoch(s) behind, exceeding its promise of {bound}")
            }
            Freshness::Undetermined { synced_at, bound } => write!(
                f,
                "mirror synced at epoch {synced_at} claiming {bound}, with no reference epoch to check it against"
            ),
            Freshness::AheadOfReference {
                synced_at,
                reference,
            } => write!(
                f,
                "mirror claims a sync at epoch {synced_at}, later than the reference epoch {reference}"
            ),
        }
    }
}

/// What a consumer is willing to accept from a mirror.
///
/// The default is the strict one: bounds must be checkable and met. An air-gapped deployment
/// relaxes it explicitly, and because the relaxation is a value it ends up recorded in the
/// resolution rather than living in somebody's head.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessPolicy {
    /// Refuse anything but the origin. Rarely satisfiable offline, which is the point of having it
    /// be a setting rather than an assumption.
    pub require_authority: bool,
    /// Proceed when the bound could not be checked. This is the offline switch.
    pub accept_undetermined: bool,
    /// Proceed when the mirror is past the bound it declared.
    pub accept_beyond_bound: bool,
    /// An additional ceiling the consumer imposes, independent of what the mirror promised.
    pub max_accepted_lag: Option<u64>,
}

impl FreshnessPolicy {
    /// Everything must come from the origin.
    pub const AUTHORITY_ONLY: FreshnessPolicy = FreshnessPolicy {
        require_authority: true,
        accept_undetermined: false,
        accept_beyond_bound: false,
        max_accepted_lag: None,
    };

    /// The air-gapped setting: mirrors are fine and unverifiable currency is accepted, knowingly.
    pub const OFFLINE: FreshnessPolicy = FreshnessPolicy {
        require_authority: false,
        accept_undetermined: true,
        accept_beyond_bound: true,
        max_accepted_lag: None,
    };

    pub fn check(&self, freshness: &Freshness) -> Result<(), MirrorError> {
        if self.require_authority && !freshness.is_from_authority() {
            return Err(MirrorError::AuthorityRequired {
                observed: freshness.to_string(),
            });
        }
        match freshness {
            Freshness::Authoritative => Ok(()),
            Freshness::WithinBound { lag, .. } => self.check_lag(*lag, freshness),
            Freshness::BeyondBound { lag, bound, .. } => {
                if !self.accept_beyond_bound {
                    return Err(MirrorError::BeyondDeclaredBound {
                        lag: *lag,
                        bound: bound.max_lag_epochs,
                    });
                }
                self.check_lag(*lag, freshness)
            }
            Freshness::Undetermined { .. } | Freshness::AheadOfReference { .. } => {
                if self.accept_undetermined {
                    Ok(())
                } else {
                    Err(MirrorError::CurrencyUndetermined {
                        detail: freshness.to_string(),
                    })
                }
            }
        }
    }

    fn check_lag(&self, lag: u64, freshness: &Freshness) -> Result<(), MirrorError> {
        match self.max_accepted_lag {
            Some(ceiling) if lag > ceiling => Err(MirrorError::LagExceedsConsumerCeiling {
                lag,
                ceiling,
                detail: freshness.to_string(),
            }),
            _ => Ok(()),
        }
    }
}

/// Why an answer from a mirror was not acceptable, or was not an answer at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MirrorError {
    #[error("the consumer requires an authoritative answer and this one is {observed}")]
    AuthorityRequired { observed: String },

    #[error("the mirror is {lag} epoch(s) behind, past its own declared bound of {bound}")]
    BeyondDeclaredBound { lag: u64, bound: u64 },

    #[error(
        "the mirror is {lag} epoch(s) behind, past the consumer's ceiling of {ceiling} ({detail})"
    )]
    LagExceedsConsumerCeiling {
        lag: u64,
        ceiling: u64,
        detail: String,
    },

    #[error("currency could not be established: {detail}")]
    CurrencyUndetermined { detail: String },

    #[error(
        "{mirror} answered {subject} with digest {mirror_digest}, but {origin} says {origin_digest}: \
         this is not a stale copy, it is a different artifact under the same name"
    )]
    Divergent {
        subject: String,
        mirror: RegistryId,
        origin: RegistryId,
        mirror_digest: String,
        origin_digest: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> RegistryId {
        RegistryId::parse(text).expect("parses")
    }

    fn mirror_synced_at(epoch: u64, bound: u64) -> Replication {
        Replication::mirror(id("origin"), Epoch(epoch), StalenessBound::epochs(bound))
    }

    #[test]
    fn a_resolution_against_a_stale_mirror_is_not_reported_as_current() {
        let stale = mirror_synced_at(2, 1).freshness(Some(Epoch(9)));
        assert!(matches!(stale, Freshness::BeyondBound { lag: 7, .. }));
        assert!(!stale.is_from_authority());
        assert!(!stale.is_within_declared_bound());
    }

    #[test]
    fn a_fresh_mirror_and_a_stale_mirror_are_distinct_outcomes_and_not_a_flag() {
        let fresh = mirror_synced_at(8, 4).freshness(Some(Epoch(9)));
        let stale = mirror_synced_at(2, 4).freshness(Some(Epoch(9)));
        assert_ne!(fresh, stale);
        assert!(fresh.is_within_declared_bound());
        assert!(!stale.is_within_declared_bound());
        assert_eq!(fresh.lag(), Some(1));
        assert_eq!(stale.lag(), Some(7));
    }

    #[test]
    fn a_fresh_mirror_is_still_not_the_authority() {
        let fresh = mirror_synced_at(9, 4).freshness(Some(Epoch(9)));
        assert!(fresh.is_within_declared_bound());
        assert!(!fresh.is_from_authority());
    }

    #[test]
    fn without_a_reference_epoch_currency_is_undetermined_rather_than_assumed_good() {
        let unknown = mirror_synced_at(2, 1).freshness(None);
        assert!(matches!(unknown, Freshness::Undetermined { .. }));
        assert!(!unknown.is_within_declared_bound());
        assert!(unknown.is_undetermined());
    }

    #[test]
    fn the_origin_needs_no_reference_epoch_to_be_authoritative() {
        assert_eq!(
            Replication::Origin.freshness(None),
            Freshness::Authoritative
        );
        assert_eq!(
            Replication::Origin.freshness(Some(Epoch(1000))),
            Freshness::Authoritative
        );
    }

    #[test]
    fn a_mirror_synced_after_the_reference_epoch_is_reported_rather_than_rounded_to_zero_lag() {
        let ahead = mirror_synced_at(20, 1).freshness(Some(Epoch(9)));
        assert!(matches!(ahead, Freshness::AheadOfReference { .. }));
        assert!(!ahead.is_within_declared_bound());
    }

    #[test]
    fn the_default_policy_refuses_an_undetermined_answer_and_the_offline_policy_accepts_it() {
        let unknown = mirror_synced_at(2, 1).freshness(None);
        assert!(matches!(
            FreshnessPolicy::default().check(&unknown),
            Err(MirrorError::CurrencyUndetermined { .. })
        ));
        assert!(FreshnessPolicy::OFFLINE.check(&unknown).is_ok());
    }

    #[test]
    fn accepting_an_undetermined_answer_is_a_recorded_decision_and_not_a_default() {
        let policy = FreshnessPolicy::default();
        assert!(!policy.accept_undetermined);
        assert!(!policy.accept_beyond_bound);
        assert!(!policy.require_authority);
    }

    #[test]
    fn a_consumer_ceiling_binds_even_when_the_mirror_is_inside_its_own_promise() {
        let generous = mirror_synced_at(0, 100).freshness(Some(Epoch(50)));
        assert!(generous.is_within_declared_bound());
        let policy = FreshnessPolicy {
            max_accepted_lag: Some(5),
            ..FreshnessPolicy::default()
        };
        assert!(matches!(
            policy.check(&generous),
            Err(MirrorError::LagExceedsConsumerCeiling {
                lag: 50,
                ceiling: 5,
                ..
            })
        ));
    }

    #[test]
    fn an_authority_only_policy_refuses_every_mirror_however_fresh() {
        let perfect = mirror_synced_at(9, 0).freshness(Some(Epoch(9)));
        assert!(matches!(perfect, Freshness::WithinBound { lag: 0, .. }));
        assert!(matches!(
            FreshnessPolicy::AUTHORITY_ONLY.check(&perfect),
            Err(MirrorError::AuthorityRequired { .. })
        ));
        assert!(FreshnessPolicy::AUTHORITY_ONLY
            .check(&Freshness::Authoritative)
            .is_ok());
    }

    #[test]
    fn freshness_survives_a_json_round_trip_with_its_variant_intact() {
        let stale = mirror_synced_at(2, 1).freshness(Some(Epoch(9)));
        let text = serde_json::to_string(&stale).expect("serialises");
        assert!(text.contains("beyond_bound"));
        let back: Freshness = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, stale);
    }
}
