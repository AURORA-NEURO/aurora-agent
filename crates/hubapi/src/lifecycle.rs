//! Yanking, withdrawal and deprecation — the three ways a published thing stops being recommended.
//!
//! Blueprint 10.04 lists the channels `draft; preview; stable; deprecated; withdrawn` and adds
//! that "a version may move channels without changing its digest". 10.05 (Pack Publication and
//! Lifecycle) and 10.20 (Supply Chain Security and Revocation) both want revocation to propagate.
//! None of the three says what a consumer that already depends on a yanked version is supposed to
//! do, which is the only genuinely hard question in the list.
//!
//! # Yank and withdrawal are not the same act
//!
//! A **yank** ([`Availability::Yanked`]) removes a version from consideration for *new* dependents
//! and leaves it resolvable for anything that already names it. That asymmetry is not a
//! convenience: it is the only rule under which a yank does not retroactively break a build that
//! was correct yesterday. A registry that made a yanked version unresolvable would be rewriting
//! the past, and 10.08's invariant — published results cannot be retroactively rewritten — would
//! fall with it.
//!
//! A **withdrawal** ([`Availability::Withdrawn`]) refuses everyone, pinned or not. It exists
//! because 10.20 needs a way to say "this artifact is unsafe" and a yank cannot say that: a yank's
//! whole point is that existing dependents proceed. Withdrawal therefore *does* break
//! reproducibility, deliberately and loudly, and carries an advisory saying so. The distinction
//! matters most in the failure: a pinned build hitting a withdrawal must stop with the advisory in
//! hand, never silently slide to a neighbouring version.
//!
//! There is [`PackLifecycle::unyank`] and there is deliberately no `unwithdraw`. Reversing a
//! security withdrawal is a new publication with a new digest, because everything downstream that
//! recorded the withdrawal has already acted on it.
//!
//! # Deprecation is `bioprism-governance`'s ladder, not a second one
//!
//! Deprecation applies to a *name* — the whole pack line — where availability applies to a
//! *version*. Rather than invent a parallel state machine, this module holds a
//! [`bioprism_governance::DeprecationLedger`] keyed by pack name and maps its four stages onto
//! resolution:
//!
//! | Stage | Effect on resolution |
//! |---|---|
//! | `active` | none |
//! | `deprecated` | resolves, with an advisory naming the replacement |
//! | `sunset` | resolves for existing dependents only |
//! | `removed` | refused |
//!
//! That inherits governance's guarantees for free: a stage is never skipped, every step carries a
//! reason and a replacement, and the ladder advances on **operator epochs** rather than on a
//! clock, because there is no clock in this workspace and a lifecycle that moved when the machine's
//! date changed would not be reproducible.
//!
//! # Not implemented
//!
//! No advisory distribution, no revocation feed, no key rotation, no scanning. 10.20 asks for all
//! four. What is here is the decision those mechanisms would carry: given a lifecycle state and a
//! reason for asking, may this version be used.

use crate::name::{PackName, Version};
use bioprism_governance::{
    DeprecationError, DeprecationLedger, LifecyclePolicy, Replacement, Stage, Transition,
};
use bioprism_hub::Epoch;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Why a caller is asking about a version.
///
/// Resolution cannot be a pure function of the catalog, because the same catalog must answer
/// differently for "pick me something" and "I already committed to this". Making the reason an
/// explicit argument is what keeps that from becoming an implicit mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "intent", rename_all = "snake_case")]
pub enum Intent {
    /// Choosing a version for the first time. Yanked versions are not candidates.
    NewDependent,
    /// Honouring a version already recorded in a lockfile, a manifest or a published result.
    /// Yanked versions remain resolvable; the yank is reported, not enforced.
    ExistingDependent,
}

/// The state of one published version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
pub enum Availability {
    Available,
    Yanked {
        reason: String,
        epoch: Epoch,
    },
    Withdrawn {
        reason: String,
        advisory: String,
        epoch: Epoch,
    },
}

impl Availability {
    /// Whether the version is under no restriction at all.
    ///
    /// Note what this deliberately is *not*: a decision about whether the version may be used. That
    /// depends on why the caller is asking and is [`PackLifecycle::admits`]. A predicate here that
    /// answered it would answer it for one intent and be silently wrong for the other.
    pub fn is_available(&self) -> bool {
        matches!(self, Availability::Available)
    }
}

impl fmt::Display for Availability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Availability::Available => f.write_str("available"),
            Availability::Yanked { reason, epoch } => {
                write!(f, "yanked at epoch {epoch}: {reason}")
            }
            Availability::Withdrawn {
                reason,
                advisory,
                epoch,
            } => write!(f, "withdrawn at epoch {epoch}: {reason} ({advisory})"),
        }
    }
}

/// What a caller is told when a version is usable but not unremarkable.
///
/// A note is never absent when something is true of the version: a yanked-but-pinned resolution
/// carries [`Note::YankedButPinned`] rather than being reported as ordinary. That is the whole
/// point — a build honouring a yank silently is a build whose operator does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "note", rename_all = "snake_case")]
pub enum Note {
    /// The version is yanked and is being resolved only because the caller already depends on it.
    YankedButPinned { reason: String, epoch: Epoch },
    /// The pack line is deprecated. Resolution proceeds.
    Deprecated {
        stage: String,
        replacement: String,
        reason: String,
    },
}

/// A version cleared for use, with everything true of it that a caller ought to know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Admission {
    pub name: PackName,
    pub version: Version,
    pub notes: Vec<Note>,
}

impl Admission {
    pub fn is_unremarkable(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn is_yanked(&self) -> bool {
        self.notes
            .iter()
            .any(|note| matches!(note, Note::YankedButPinned { .. }))
    }

    pub fn is_deprecated(&self) -> bool {
        self.notes
            .iter()
            .any(|note| matches!(note, Note::Deprecated { .. }))
    }
}

/// Every lifecycle fact a registry holds: per-version availability and per-name deprecation.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackLifecycle {
    availability: BTreeMap<String, Availability>,
    deprecation: DeprecationLedger,
}

impl PackLifecycle {
    pub fn new() -> Self {
        PackLifecycle::default()
    }

    fn key(name: &PackName, version: &Version) -> String {
        format!("{name}@{version}")
    }

    /// Marks a version yanked. Existing dependents keep resolving it.
    pub fn yank(
        &mut self,
        name: &PackName,
        version: Version,
        reason: impl Into<String>,
        epoch: Epoch,
    ) -> Result<(), LifecycleError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(LifecycleError::ReasonMissing {
                subject: PackLifecycle::key(name, &version),
                action: "yank",
            });
        }
        let key = PackLifecycle::key(name, &version);
        if let Some(Availability::Withdrawn { .. }) = self.availability.get(&key) {
            return Err(LifecycleError::AlreadyWithdrawn { subject: key });
        }
        self.availability
            .insert(key, Availability::Yanked { reason, epoch });
        Ok(())
    }

    /// Restores a yanked version. Refused for a withdrawal, which has no reverse.
    pub fn unyank(&mut self, name: &PackName, version: Version) -> Result<(), LifecycleError> {
        let key = PackLifecycle::key(name, &version);
        match self.availability.get(&key) {
            Some(Availability::Withdrawn { .. }) => {
                Err(LifecycleError::WithdrawalIsNotReversible { subject: key })
            }
            _ => {
                self.availability.insert(key, Availability::Available);
                Ok(())
            }
        }
    }

    /// Withdraws a version from everyone. This breaks pinned builds on purpose.
    pub fn withdraw(
        &mut self,
        name: &PackName,
        version: Version,
        reason: impl Into<String>,
        advisory: impl Into<String>,
        epoch: Epoch,
    ) -> Result<(), LifecycleError> {
        let reason = reason.into();
        let advisory = advisory.into();
        let key = PackLifecycle::key(name, &version);
        if reason.trim().is_empty() {
            return Err(LifecycleError::ReasonMissing {
                subject: key,
                action: "withdraw",
            });
        }
        if advisory.trim().is_empty() {
            return Err(LifecycleError::AdvisoryMissing { subject: key });
        }
        self.availability.insert(
            key,
            Availability::Withdrawn {
                reason,
                advisory,
                epoch,
            },
        );
        Ok(())
    }

    pub fn availability(&self, name: &PackName, version: &Version) -> Availability {
        self.availability
            .get(&PackLifecycle::key(name, version))
            .cloned()
            .unwrap_or(Availability::Available)
    }

    /// Registers a pack line with the deprecation ladder in its `active` stage.
    pub fn declare(&mut self, name: &PackName, epoch: Epoch) -> Result<(), LifecycleError> {
        self.deprecation
            .declare(name.to_string(), epoch.get())
            .map_err(LifecycleError::from)
    }

    /// Advances a pack line one stage along `bioprism-governance`'s ladder.
    pub fn advance(
        &mut self,
        name: &PackName,
        to: Stage,
        epoch: Epoch,
        reason: impl Into<String>,
        replacement: Replacement,
        policy: &LifecyclePolicy,
    ) -> Result<(), LifecycleError> {
        self.deprecation
            .advance(
                &name.to_string(),
                Transition::new(to, epoch.get(), reason, replacement),
                policy,
            )
            .map_err(LifecycleError::from)
    }

    pub fn stage(&self, name: &PackName) -> Stage {
        self.deprecation
            .stage_of(&name.to_string())
            .unwrap_or(Stage::Active)
    }

    fn deprecation_note(&self, name: &PackName) -> Option<Note> {
        let record = self.deprecation.record(&name.to_string())?;
        let last = record.history().last()?;
        Some(Note::Deprecated {
            stage: record.stage().to_string(),
            replacement: last.replacement.to_string(),
            reason: last.reason.clone(),
        })
    }

    /// Decides whether a version may be used, and says everything true of it if so.
    ///
    /// The two refusals are distinct types of event and are reported as distinct errors, because a
    /// caller can respond to [`LifecycleError::YankedForNewDependents`] by choosing another version
    /// and cannot respond to [`LifecycleError::VersionWithdrawn`] at all.
    pub fn admits(
        &self,
        name: &PackName,
        version: Version,
        intent: Intent,
    ) -> Result<Admission, LifecycleError> {
        let subject = PackLifecycle::key(name, &version);
        let mut notes = Vec::new();

        match self.availability(name, &version) {
            Availability::Available => {}
            Availability::Yanked { reason, epoch } => match intent {
                Intent::NewDependent => {
                    return Err(LifecycleError::YankedForNewDependents { subject, reason })
                }
                Intent::ExistingDependent => notes.push(Note::YankedButPinned { reason, epoch }),
            },
            Availability::Withdrawn {
                reason, advisory, ..
            } => {
                return Err(LifecycleError::VersionWithdrawn {
                    subject,
                    reason,
                    advisory,
                })
            }
        }

        match self.stage(name) {
            Stage::Active => {}
            Stage::Deprecated => notes.extend(self.deprecation_note(name)),
            Stage::Sunset => match intent {
                Intent::NewDependent => {
                    return Err(LifecycleError::SunsetForNewDependents {
                        name: name.to_string(),
                    })
                }
                Intent::ExistingDependent => notes.extend(self.deprecation_note(name)),
            },
            Stage::Removed => {
                return Err(LifecycleError::NameRemoved {
                    name: name.to_string(),
                })
            }
        }

        Ok(Admission {
            name: name.clone(),
            version,
            notes,
        })
    }
}

/// Why a version may not be used, or why a lifecycle step did not happen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("{subject} is yanked and cannot be chosen by a new dependent: {reason}")]
    YankedForNewDependents { subject: String, reason: String },

    #[error(
        "{subject} is withdrawn and is refused even to existing dependents: {reason} ({advisory})"
    )]
    VersionWithdrawn {
        subject: String,
        reason: String,
        advisory: String,
    },

    #[error("{name} is at sunset: existing dependents resolve, new ones do not")]
    SunsetForNewDependents { name: String },

    #[error("{name} has been removed and no version of it resolves")]
    NameRemoved { name: String },

    #[error("a {action} of {subject} must state a reason")]
    ReasonMissing {
        subject: String,
        action: &'static str,
    },

    #[error("a withdrawal of {subject} must carry an advisory")]
    AdvisoryMissing { subject: String },

    #[error("{subject} is withdrawn; a yank cannot weaken a withdrawal")]
    AlreadyWithdrawn { subject: String },

    #[error("{subject} is withdrawn; reversing that is a new publication, not a state change")]
    WithdrawalIsNotReversible { subject: String },

    #[error(transparent)]
    Deprecation(#[from] DeprecationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name() -> PackName {
        PackName::parse("bioprism/onco-tp53").expect("parses")
    }

    fn v(major: u64, minor: u64, patch: u64) -> Version {
        Version::new(major, minor, patch)
    }

    fn deprecated_line() -> PackLifecycle {
        let mut lifecycle = PackLifecycle::new();
        lifecycle.declare(&name(), Epoch(0)).expect("declared");
        lifecycle
            .advance(
                &name(),
                Stage::Deprecated,
                Epoch(1),
                "the TP53 parent set was rebuilt against a corrected reference",
                Replacement::field("bioprism/onco-tp53-r2"),
                &LifecyclePolicy::default(),
            )
            .expect("advances");
        lifecycle
    }

    #[test]
    fn a_yanked_version_stays_resolvable_for_something_that_already_depends_on_it() {
        let mut lifecycle = PackLifecycle::new();
        lifecycle
            .yank(
                &name(),
                v(1, 2, 0),
                "instance 41 had a leaked label",
                Epoch(7),
            )
            .expect("yanks");

        let admission = lifecycle
            .admits(&name(), v(1, 2, 0), Intent::ExistingDependent)
            .expect("a yank does not rewrite yesterday's build");
        assert!(admission.is_yanked());
        assert!(matches!(
            admission.notes.as_slice(),
            [Note::YankedButPinned { epoch, .. }] if *epoch == Epoch(7)
        ));
    }

    #[test]
    fn a_yanked_version_is_refused_to_a_new_dependent() {
        let mut lifecycle = PackLifecycle::new();
        lifecycle
            .yank(
                &name(),
                v(1, 2, 0),
                "instance 41 had a leaked label",
                Epoch(7),
            )
            .expect("yanks");
        let error = lifecycle
            .admits(&name(), v(1, 2, 0), Intent::NewDependent)
            .expect_err("nothing new should start depending on it");
        assert!(matches!(
            error,
            LifecycleError::YankedForNewDependents { .. }
        ));
    }

    #[test]
    fn a_resolution_that_honours_a_yank_is_never_reported_as_unremarkable() {
        let mut lifecycle = PackLifecycle::new();
        lifecycle
            .yank(&name(), v(1, 2, 0), "leaked label", Epoch(7))
            .expect("yanks");
        let admission = lifecycle
            .admits(&name(), v(1, 2, 0), Intent::ExistingDependent)
            .expect("resolves");
        assert!(!admission.is_unremarkable());
    }

    #[test]
    fn a_withdrawn_version_is_refused_to_a_pinned_dependent_with_the_advisory_attached() {
        let mut lifecycle = PackLifecycle::new();
        lifecycle
            .withdraw(
                &name(),
                v(1, 2, 0),
                "the archive shipped a live credential",
                "BIOPRISM-2026-04",
                Epoch(9),
            )
            .expect("withdraws");
        let error = lifecycle
            .admits(&name(), v(1, 2, 0), Intent::ExistingDependent)
            .expect_err("a withdrawal breaks the build on purpose");
        assert!(matches!(
            error,
            LifecycleError::VersionWithdrawn { ref advisory, .. } if advisory == "BIOPRISM-2026-04"
        ));
    }

    #[test]
    fn a_withdrawal_cannot_be_reversed_by_unyanking_it() {
        let mut lifecycle = PackLifecycle::new();
        lifecycle
            .withdraw(
                &name(),
                v(1, 2, 0),
                "live credential",
                "BIOPRISM-2026-04",
                Epoch(9),
            )
            .expect("withdraws");
        assert!(matches!(
            lifecycle.unyank(&name(), v(1, 2, 0)),
            Err(LifecycleError::WithdrawalIsNotReversible { .. })
        ));
        assert!(matches!(
            lifecycle.yank(&name(), v(1, 2, 0), "milder reason", Epoch(10)),
            Err(LifecycleError::AlreadyWithdrawn { .. })
        ));
    }

    #[test]
    fn a_yank_may_be_reversed_and_the_version_becomes_a_candidate_again() {
        let mut lifecycle = PackLifecycle::new();
        lifecycle
            .yank(&name(), v(1, 2, 0), "mistaken", Epoch(3))
            .expect("yanks");
        lifecycle.unyank(&name(), v(1, 2, 0)).expect("unyanks");
        let admission = lifecycle
            .admits(&name(), v(1, 2, 0), Intent::NewDependent)
            .expect("available again");
        assert!(admission.is_unremarkable());
    }

    #[test]
    fn a_yank_without_a_stated_reason_does_not_happen() {
        let mut lifecycle = PackLifecycle::new();
        assert!(matches!(
            lifecycle.yank(&name(), v(1, 2, 0), "   ", Epoch(3)),
            Err(LifecycleError::ReasonMissing { action: "yank", .. })
        ));
        assert!(lifecycle.availability(&name(), &v(1, 2, 0)).is_available());
    }

    #[test]
    fn a_withdrawal_without_an_advisory_does_not_happen() {
        let mut lifecycle = PackLifecycle::new();
        assert!(matches!(
            lifecycle.withdraw(&name(), v(1, 2, 0), "unsafe", "  ", Epoch(3)),
            Err(LifecycleError::AdvisoryMissing { .. })
        ));
    }

    #[test]
    fn a_deprecated_pack_line_still_resolves_and_says_what_replaces_it() {
        let lifecycle = deprecated_line();
        let admission = lifecycle
            .admits(&name(), v(1, 2, 0), Intent::NewDependent)
            .expect("deprecated is a recommendation, not a refusal");
        assert!(admission.is_deprecated());
        assert!(matches!(
            admission.notes.as_slice(),
            [Note::Deprecated { replacement, .. }] if replacement.contains("onco-tp53-r2")
        ));
    }

    #[test]
    fn a_pack_line_at_sunset_resolves_for_existing_dependents_and_not_for_new_ones() {
        let mut lifecycle = deprecated_line();
        lifecycle
            .advance(
                &name(),
                Stage::Sunset,
                Epoch(2),
                "no new submissions accepted against this line",
                Replacement::field("bioprism/onco-tp53-r2"),
                &LifecyclePolicy::default(),
            )
            .expect("advances");

        assert!(lifecycle
            .admits(&name(), v(1, 2, 0), Intent::ExistingDependent)
            .is_ok());
        assert!(matches!(
            lifecycle.admits(&name(), v(1, 2, 0), Intent::NewDependent),
            Err(LifecycleError::SunsetForNewDependents { .. })
        ));
    }

    #[test]
    fn a_removed_pack_line_resolves_for_nobody() {
        let mut lifecycle = deprecated_line();
        for (stage, epoch) in [(Stage::Sunset, 2), (Stage::Removed, 3)] {
            lifecycle
                .advance(
                    &name(),
                    stage,
                    Epoch(epoch),
                    "the line is gone",
                    Replacement::field("bioprism/onco-tp53-r2"),
                    &LifecyclePolicy::default(),
                )
                .expect("advances");
        }
        assert!(matches!(
            lifecycle.admits(&name(), v(1, 2, 0), Intent::ExistingDependent),
            Err(LifecycleError::NameRemoved { .. })
        ));
    }

    #[test]
    fn the_deprecation_ladder_is_governances_and_still_refuses_a_skipped_stage() {
        let mut lifecycle = PackLifecycle::new();
        lifecycle.declare(&name(), Epoch(0)).expect("declared");
        let error = lifecycle
            .advance(
                &name(),
                Stage::Removed,
                Epoch(1),
                "cleanup",
                Replacement::field("bioprism/onco-tp53-r2"),
                &LifecyclePolicy::default(),
            )
            .expect_err("this crate does not get its own, laxer ladder");
        assert!(matches!(error, LifecycleError::Deprecation(_)));
        assert_eq!(lifecycle.stage(&name()), Stage::Active);
    }

    #[test]
    fn a_pack_line_nobody_declared_is_active_rather_than_unknown() {
        let lifecycle = PackLifecycle::new();
        assert_eq!(lifecycle.stage(&name()), Stage::Active);
        assert!(lifecycle
            .admits(&name(), v(1, 0, 0), Intent::NewDependent)
            .expect("silence is not a refusal")
            .is_unremarkable());
    }

    #[test]
    fn availability_is_per_version_and_deprecation_is_per_name() {
        let mut lifecycle = deprecated_line();
        lifecycle
            .yank(&name(), v(1, 2, 0), "leaked label", Epoch(5))
            .expect("yanks");
        assert!(lifecycle.availability(&name(), &v(1, 3, 0)).is_available());
        let other = lifecycle
            .admits(&name(), v(1, 3, 0), Intent::NewDependent)
            .expect("the sibling version is unaffected by the yank");
        assert!(!other.is_yanked());
        assert!(other.is_deprecated());
    }
}
