//! Moderation states, transitions, and the append-only ledger they live in.
//!
//! Blueprint 34.15 requires the public surface to support `under-review`, `disputed`, `withdrawn`
//! and `non-reproducible` as first-class states, 34.16 lists "appeal fairness" as a product
//! metric, and 36.17 requires an "append-only event log" over "policy decisions and result
//! publication". 36.18 adds the hard part: withdrawal must propagate to derived artifacts without
//! erasing the record that the artifact existed.
//!
//! # Why there is no `remove`
//!
//! A hub whose operator can make an entry vanish cannot be audited, because the absence of an
//! entry and the deletion of an entry are then indistinguishable from the outside. So withdrawal
//! is a transition, not a deletion, and it leaves a [`Tombstone`]: the identifier, the content
//! digest, who withdrew it, when, why, and every state it passed through.
//!
//! The tombstone deliberately does *not* retain the submission's prose. 36.18 covers participant
//! withdrawal, and a "tombstone" that reproduced the withdrawn content would defeat the withdrawal
//! it records. What survives is enough to prove the entry existed and to detect a later
//! contradiction; not enough to republish it.
//!
//! # Why epochs must not go backwards
//!
//! [`ModerationLedger`] refuses an event whose epoch precedes the last recorded one. An audit log
//! that accepts backdated entries records a history that was chosen rather than observed. This is
//! also why the ledger takes an operator-supplied [`Epoch`] instead of reading a clock: a clock
//! read inside the ledger cannot be replayed, and a replayable ledger is the point.
//!
//! # What is not implemented
//!
//! No storage, no durability, no concurrency control, no signatures. This is the state machine and
//! its invariants; persisting the event vector and signing it are the deployment's job.

use crate::error::HubError;
use crate::id::{Epoch, SubmissionId, SubmitterId};
use crate::submission::{Submission, VerificationStatus};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Where a submission stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModerationState {
    /// Received and contract-checked. Not published.
    Submitted,
    /// A reviewer has it. Also the state a disputed published entry returns to, which is why
    /// re-entry from [`ModerationState::Accepted`] requires a recorded reason.
    UnderReview,
    /// Published. The only state whose entries a leaderboard will rank.
    Accepted,
    /// Refused, with a reason on the record so it can be appealed.
    Rejected,
    /// Pulled by the submitter or the operator. Terminal, and leaves a [`Tombstone`].
    Withdrawn,
    /// Replaced by a named successor. 34.15: "corrections preserve prior versions".
    Superseded,
}

impl ModerationState {
    pub fn as_str(self) -> &'static str {
        match self {
            ModerationState::Submitted => "submitted",
            ModerationState::UnderReview => "under-review",
            ModerationState::Accepted => "accepted",
            ModerationState::Rejected => "rejected",
            ModerationState::Withdrawn => "withdrawn",
            ModerationState::Superseded => "superseded",
        }
    }

    /// Withdrawal is the only state with no way out. A withdrawn artifact that could be restored
    /// would make the tombstone a lie.
    pub fn is_terminal(self) -> bool {
        self == ModerationState::Withdrawn
    }

    /// Whether entries in this state may appear in a ranking.
    pub fn is_publishable(self) -> bool {
        self == ModerationState::Accepted
    }

    /// The legal successors of this state.
    pub fn successors(self) -> &'static [ModerationState] {
        use ModerationState::*;
        match self {
            Submitted => &[UnderReview, Rejected, Withdrawn],
            UnderReview => &[Accepted, Rejected, Withdrawn],
            Accepted => &[UnderReview, Superseded, Withdrawn],
            Rejected => &[UnderReview, Withdrawn],
            Superseded => &[Withdrawn],
            Withdrawn => &[],
        }
    }

    pub fn permits(self, to: ModerationState) -> bool {
        self.successors().contains(&to)
    }

    /// Transitions that must carry a reason: anything that removes standing, and anything that
    /// reopens a settled decision. Acceptance after review is the only silent good-news path, and
    /// even that is attributed to an actor.
    pub fn requires_reason(self, to: ModerationState) -> bool {
        matches!(to, ModerationState::Rejected | ModerationState::Withdrawn)
            || (to == ModerationState::UnderReview && self != ModerationState::Submitted)
    }
}

impl fmt::Display for ModerationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What happened, as opposed to what state it left behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum EventKind {
    /// The submission was placed on the ledger.
    Opened,
    Transition {
        from: ModerationState,
        to: ModerationState,
    },
    /// Verification moved, by a reviewer who is not the submitter.
    Attestation {
        from: VerificationStatus,
        to: VerificationStatus,
    },
}

/// One immutable line of the audit log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModerationEvent {
    pub submission: SubmissionId,
    pub kind: EventKind,
    /// Who did it. Free-form: the hub is not an identity provider (see [`crate::id`]).
    pub actor: String,
    pub at: Epoch,
    pub reason: Option<String>,
    /// Present only on a transition into [`ModerationState::Superseded`].
    pub superseded_by: Option<SubmissionId>,
}

/// The inputs a transition needs from the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub actor: String,
    pub at: Epoch,
    pub reason: Option<String>,
    pub superseded_by: Option<SubmissionId>,
}

impl Decision {
    pub fn by(actor: impl Into<String>, at: Epoch) -> Decision {
        Decision {
            actor: actor.into(),
            at,
            reason: None,
            superseded_by: None,
        }
    }

    pub fn because(mut self, reason: impl Into<String>) -> Decision {
        self.reason = Some(reason.into());
        self
    }

    pub fn superseded_by(mut self, successor: SubmissionId) -> Decision {
        self.superseded_by = Some(successor);
        self
    }
}

/// What remains of a withdrawn submission.
///
/// Enough to prove the entry existed, to detect a later record that contradicts it, and to follow
/// derivations that cited its digest. Not enough to reconstruct it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    pub submission: SubmissionId,
    pub submitter: SubmitterId,
    pub content: ContentHash,
    pub withdrawn_at: Epoch,
    pub actor: String,
    pub reason: String,
    /// Every state the submission occupied before withdrawal, in order.
    pub states_traversed: Vec<ModerationState>,
}

/// A submission and everything the hub has decided about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModerationRecord {
    /// The submission exactly as accepted. Its `verification` field is the as-submitted value and
    /// stays [`VerificationStatus::SelfReported`] forever; the current status is on this record.
    pub submission: Submission,
    pub state: ModerationState,
    pub verification: VerificationStatus,
    pub history: Vec<ModerationEvent>,
    /// Present iff `state == Withdrawn`. The content is gone from the record; this is what is left.
    pub tombstone: Option<Tombstone>,
}

impl ModerationRecord {
    /// The reason recorded with the most recent transition into the current state, if any. A
    /// rejected record always has one.
    pub fn current_reason(&self) -> Option<&str> {
        self.history
            .iter()
            .rev()
            .find(|e| matches!(e.kind, EventKind::Transition { to, .. } if to == self.state))
            .and_then(|e| e.reason.as_deref())
    }

    /// The named successor, if this record was superseded.
    pub fn superseded_by(&self) -> Option<&SubmissionId> {
        self.history
            .iter()
            .rev()
            .find(|e| {
                matches!(
                    e.kind,
                    EventKind::Transition {
                        to: ModerationState::Superseded,
                        ..
                    }
                )
            })
            .and_then(|e| e.superseded_by.as_ref())
    }
}

/// The append-only moderation log.
///
/// There is no method that removes a record, and that omission is the design. See the module note.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModerationLedger {
    records: BTreeMap<SubmissionId, ModerationRecord>,
    events: Vec<ModerationEvent>,
    last_epoch: Epoch,
}

impl ModerationLedger {
    pub fn new() -> ModerationLedger {
        ModerationLedger::default()
    }

    /// The epoch of the most recent event. Nothing may be recorded before it.
    pub fn last_epoch(&self) -> Epoch {
        self.last_epoch
    }

    /// Every event ever recorded, in the order recorded.
    pub fn events(&self) -> &[ModerationEvent] {
        &self.events
    }

    pub fn record(&self, id: &SubmissionId) -> Option<&ModerationRecord> {
        self.records.get(id)
    }

    pub fn state(&self, id: &SubmissionId) -> Option<ModerationState> {
        self.records.get(id).map(|r| r.state)
    }

    pub fn verification(&self, id: &SubmissionId) -> Option<VerificationStatus> {
        self.records.get(id).map(|r| r.verification)
    }

    pub fn tombstone(&self, id: &SubmissionId) -> Option<&Tombstone> {
        self.records.get(id).and_then(|r| r.tombstone.as_ref())
    }

    /// Every tombstone on the ledger, in identifier order. A transparency report reads this.
    pub fn tombstones(&self) -> Vec<&Tombstone> {
        self.records
            .values()
            .filter_map(|r| r.tombstone.as_ref())
            .collect()
    }

    pub fn history(&self, id: &SubmissionId) -> Vec<&ModerationEvent> {
        self.events.iter().filter(|e| &e.submission == id).collect()
    }

    /// Identifiers currently in [`ModerationState::Accepted`].
    pub fn published(&self) -> Vec<&SubmissionId> {
        self.records
            .iter()
            .filter(|(_, r)| r.state.is_publishable())
            .map(|(id, _)| id)
            .collect()
    }

    fn advance(&mut self, at: Epoch) -> Result<(), HubError> {
        if at < self.last_epoch {
            return Err(HubError::NonMonotonicEpoch {
                proposed: at.get(),
                last: self.last_epoch.get(),
            });
        }
        self.last_epoch = at;
        Ok(())
    }

    /// Place an accepted-contract submission on the ledger in [`ModerationState::Submitted`].
    pub fn open(
        &mut self,
        submission: Submission,
        actor: impl Into<String>,
        at: Epoch,
    ) -> Result<&ModerationRecord, HubError> {
        let id = submission.id.clone();
        if self.records.contains_key(&id) {
            return Err(HubError::DuplicateSubmission(id.to_string()));
        }
        self.advance(at)?;

        let event = ModerationEvent {
            submission: id.clone(),
            kind: EventKind::Opened,
            actor: actor.into(),
            at,
            reason: None,
            superseded_by: None,
        };
        let verification = submission.verification;
        let record = ModerationRecord {
            submission,
            state: ModerationState::Submitted,
            verification,
            history: vec![event.clone()],
            tombstone: None,
        };
        self.events.push(event);
        self.records.insert(id.clone(), record);
        self.records
            .get(&id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))
    }

    /// Move a submission, or refuse.
    ///
    /// Refusals: unknown identifier; a transition the state machine does not permit (including
    /// every transition out of [`ModerationState::Withdrawn`]); a missing reason where one is
    /// required; supersession with no named successor or with itself; a backdated epoch.
    pub fn transition(
        &mut self,
        id: &SubmissionId,
        to: ModerationState,
        decision: Decision,
    ) -> Result<&ModerationRecord, HubError> {
        let from = self
            .records
            .get(id)
            .map(|r| r.state)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))?;

        if !from.permits(to) {
            return Err(HubError::IllegalTransition {
                submission: id.to_string(),
                from,
                to,
            });
        }

        let reason = decision.reason.clone().filter(|r| !r.trim().is_empty());
        if from.requires_reason(to) && reason.is_none() {
            return Err(HubError::MissingReason { to });
        }

        if to == ModerationState::Superseded {
            match decision.superseded_by.as_ref() {
                None => return Err(HubError::MissingSupersedingSubmission),
                Some(successor) if successor == id => {
                    return Err(HubError::SelfSupersession(id.to_string()))
                }
                Some(_) => {}
            }
        }

        self.advance(decision.at)?;

        let event = ModerationEvent {
            submission: id.clone(),
            kind: EventKind::Transition { from, to },
            actor: decision.actor.clone(),
            at: decision.at,
            reason: reason.clone(),
            superseded_by: decision.superseded_by.clone(),
        };

        let record = self
            .records
            .get_mut(id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))?;
        record.state = to;
        record.history.push(event.clone());
        if to == ModerationState::Withdrawn {
            let mut states_traversed: Vec<ModerationState> = record
                .history
                .iter()
                .filter_map(|e| match e.kind {
                    EventKind::Transition { from, .. } => Some(from),
                    _ => None,
                })
                .collect();
            states_traversed.push(to);
            record.tombstone = Some(Tombstone {
                submission: id.clone(),
                submitter: record.submission.submitter.clone(),
                content: record.submission.content.clone(),
                withdrawn_at: decision.at,
                actor: decision.actor,
                reason: reason.unwrap_or_default(),
                states_traversed,
            });
        }
        self.events.push(event);
        self.records
            .get(id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))
    }

    /// Raise a published submission's verification status.
    ///
    /// Refuses a self-attestation, an attestation on anything not currently published, and any
    /// attestation that is not strictly upward. Downgrades go through [`Self::revoke_attestation`]
    /// so that they carry a reason.
    pub fn attest(
        &mut self,
        id: &SubmissionId,
        to: VerificationStatus,
        actor: impl Into<String>,
        at: Epoch,
    ) -> Result<&ModerationRecord, HubError> {
        let actor = actor.into();
        let record = self
            .records
            .get(id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))?;

        if record.submission.submitter.as_str() == actor {
            return Err(HubError::SelfReview {
                actor,
                submission: id.to_string(),
            });
        }
        if !record.state.is_publishable() {
            return Err(HubError::IllegalTransition {
                submission: id.to_string(),
                from: record.state,
                to: ModerationState::Accepted,
            });
        }
        let from = record.verification;
        if to <= from {
            return Err(HubError::SelfAssertedVerification {
                claimed: to.as_str(),
            });
        }

        self.advance(at)?;
        let event = ModerationEvent {
            submission: id.clone(),
            kind: EventKind::Attestation { from, to },
            actor,
            at,
            reason: None,
            superseded_by: None,
        };
        let record = self
            .records
            .get_mut(id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))?;
        record.verification = to;
        record.history.push(event.clone());
        self.events.push(event);
        self.records
            .get(id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))
    }

    /// Drop a submission back to self-reported, with a reason. Used when a reproduction later
    /// fails; 34.15 requires that corrections "explain score changes".
    pub fn revoke_attestation(
        &mut self,
        id: &SubmissionId,
        actor: impl Into<String>,
        at: Epoch,
        reason: impl Into<String>,
    ) -> Result<&ModerationRecord, HubError> {
        let from = self
            .records
            .get(id)
            .map(|r| r.verification)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))?;
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(HubError::MissingReason {
                to: ModerationState::UnderReview,
            });
        }
        self.advance(at)?;
        let event = ModerationEvent {
            submission: id.clone(),
            kind: EventKind::Attestation {
                from,
                to: VerificationStatus::SelfReported,
            },
            actor: actor.into(),
            at,
            reason: Some(reason),
            superseded_by: None,
        };
        let record = self
            .records
            .get_mut(id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))?;
        record.verification = VerificationStatus::SelfReported;
        record.history.push(event.clone());
        self.events.push(event);
        self.records
            .get(id)
            .ok_or_else(|| HubError::UnknownSubmission(id.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{accepted_submission, submission_with_id};

    fn published_ledger() -> (ModerationLedger, SubmissionId) {
        let submission = accepted_submission();
        let id = submission.id.clone();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission, "hub", Epoch(1)).unwrap();
        ledger
            .transition(
                &id,
                ModerationState::UnderReview,
                Decision::by("rev-1", Epoch(2)),
            )
            .unwrap();
        ledger
            .transition(
                &id,
                ModerationState::Accepted,
                Decision::by("rev-1", Epoch(3)),
            )
            .unwrap();
        (ledger, id)
    }

    #[test]
    fn a_submission_starts_submitted_and_is_not_yet_publishable() {
        let submission = accepted_submission();
        let id = submission.id.clone();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission, "hub", Epoch(1)).unwrap();
        assert_eq!(ledger.state(&id), Some(ModerationState::Submitted));
        assert!(ledger.published().is_empty());
    }

    #[test]
    fn the_same_identifier_cannot_be_opened_twice() {
        let submission = accepted_submission();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission.clone(), "hub", Epoch(1)).unwrap();
        let err = ledger
            .open(submission, "hub", Epoch(2))
            .expect_err("duplicate");
        assert_eq!(err, HubError::DuplicateSubmission("sub-1".into()));
    }

    #[test]
    fn a_rejection_without_a_reason_is_refused() {
        let submission = accepted_submission();
        let id = submission.id.clone();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission, "hub", Epoch(1)).unwrap();
        let err = ledger
            .transition(
                &id,
                ModerationState::Rejected,
                Decision::by("rev-1", Epoch(2)),
            )
            .expect_err("no reason");
        assert_eq!(
            err,
            HubError::MissingReason {
                to: ModerationState::Rejected
            }
        );
    }

    #[test]
    fn a_rejection_records_its_reason_for_appeal() {
        let submission = accepted_submission();
        let id = submission.id.clone();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission, "hub", Epoch(1)).unwrap();
        let record = ledger
            .transition(
                &id,
                ModerationState::Rejected,
                Decision::by("rev-1", Epoch(2)).because("pack digest does not resolve"),
            )
            .unwrap();
        assert_eq!(record.state, ModerationState::Rejected);
        assert_eq!(
            record.current_reason(),
            Some("pack digest does not resolve")
        );
    }

    #[test]
    fn a_rejected_submission_can_be_reopened_on_appeal() {
        let submission = accepted_submission();
        let id = submission.id.clone();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission, "hub", Epoch(1)).unwrap();
        ledger
            .transition(
                &id,
                ModerationState::Rejected,
                Decision::by("rev-1", Epoch(2)).because("digest mismatch"),
            )
            .unwrap();
        let record = ledger
            .transition(
                &id,
                ModerationState::UnderReview,
                Decision::by("board", Epoch(3)).because("appeal accepted"),
            )
            .unwrap();
        assert_eq!(record.state, ModerationState::UnderReview);
    }

    #[test]
    fn a_withdrawn_submission_leaves_a_tombstone_and_cannot_be_deleted_or_restored() {
        let (mut ledger, id) = published_ledger();
        ledger
            .transition(
                &id,
                ModerationState::Withdrawn,
                Decision::by("lab-a", Epoch(4)).because("participant withdrawal upstream"),
            )
            .unwrap();

        let tombstone = ledger.tombstone(&id).expect("tombstone written");
        assert_eq!(tombstone.reason, "participant withdrawal upstream");
        assert_eq!(tombstone.withdrawn_at, Epoch(4));
        assert!(tombstone
            .states_traversed
            .contains(&ModerationState::Accepted));
        assert_eq!(ledger.tombstones().len(), 1);

        let err = ledger
            .transition(
                &id,
                ModerationState::Accepted,
                Decision::by("hub", Epoch(5)),
            )
            .expect_err("withdrawal is terminal");
        assert_eq!(
            err,
            HubError::IllegalTransition {
                submission: "sub-1".into(),
                from: ModerationState::Withdrawn,
                to: ModerationState::Accepted,
            }
        );
    }

    #[test]
    fn a_tombstone_does_not_reproduce_the_withdrawn_content() {
        let (mut ledger, id) = published_ledger();
        ledger
            .transition(
                &id,
                ModerationState::Withdrawn,
                Decision::by("lab-a", Epoch(4)).because("consent revoked"),
            )
            .unwrap();
        let tombstone = ledger.tombstone(&id).unwrap();
        let rendered = serde_json::to_string(tombstone).unwrap();
        assert!(
            !rendered.contains("glioma"),
            "tombstone leaked scope prose: {rendered}"
        );
        assert!(rendered.contains(tombstone.content.as_str()));
    }

    #[test]
    fn supersession_requires_a_named_successor_that_is_not_itself() {
        let (mut ledger, id) = published_ledger();
        let err = ledger
            .transition(
                &id,
                ModerationState::Superseded,
                Decision::by("hub", Epoch(4)),
            )
            .expect_err("no successor");
        assert_eq!(err, HubError::MissingSupersedingSubmission);

        let err = ledger
            .transition(
                &id,
                ModerationState::Superseded,
                Decision::by("hub", Epoch(4)).superseded_by(id.clone()),
            )
            .expect_err("self supersession");
        assert_eq!(err, HubError::SelfSupersession("sub-1".into()));

        let successor = SubmissionId::parse("sub-2").unwrap();
        let record = ledger
            .transition(
                &id,
                ModerationState::Superseded,
                Decision::by("hub", Epoch(4)).superseded_by(successor.clone()),
            )
            .unwrap();
        assert_eq!(record.superseded_by(), Some(&successor));
    }

    #[test]
    fn an_event_cannot_be_backdated_behind_the_ledger() {
        let (mut ledger, id) = published_ledger();
        let err = ledger
            .transition(
                &id,
                ModerationState::Withdrawn,
                Decision::by("hub", Epoch(1)).because("backdated"),
            )
            .expect_err("backdated event");
        assert_eq!(
            err,
            HubError::NonMonotonicEpoch {
                proposed: 1,
                last: 3
            }
        );
    }

    #[test]
    fn a_submitter_cannot_attest_their_own_submission() {
        let (mut ledger, id) = published_ledger();
        let err = ledger
            .attest(&id, VerificationStatus::Verified, "lab-a", Epoch(4))
            .expect_err("self review");
        assert_eq!(
            err,
            HubError::SelfReview {
                actor: "lab-a".into(),
                submission: "sub-1".into(),
            }
        );
    }

    #[test]
    fn verification_is_awarded_upward_and_revoked_with_a_reason() {
        let (mut ledger, id) = published_ledger();
        ledger
            .attest(&id, VerificationStatus::Reproduced, "rev-2", Epoch(4))
            .unwrap();
        assert_eq!(
            ledger.verification(&id),
            Some(VerificationStatus::Reproduced)
        );

        let err = ledger
            .attest(&id, VerificationStatus::SelfReported, "rev-2", Epoch(5))
            .expect_err("attest is not a downgrade path");
        assert!(matches!(err, HubError::SelfAssertedVerification { .. }));

        let record = ledger
            .revoke_attestation(&id, "rev-3", Epoch(6), "independent rebuild diverged")
            .unwrap();
        assert_eq!(record.verification, VerificationStatus::SelfReported);
        assert!(record
            .history
            .iter()
            .any(|e| e.reason.as_deref() == Some("independent rebuild diverged")));
    }

    #[test]
    fn an_unpublished_submission_cannot_be_attested() {
        let submission = accepted_submission();
        let id = submission.id.clone();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission, "hub", Epoch(1)).unwrap();
        let err = ledger
            .attest(&id, VerificationStatus::Verified, "rev-2", Epoch(2))
            .expect_err("not published");
        assert!(matches!(
            err,
            HubError::IllegalTransition {
                from: ModerationState::Submitted,
                ..
            }
        ));
    }

    #[test]
    fn the_ledger_round_trips_through_json_with_its_history_intact() {
        let (mut ledger, id) = published_ledger();
        ledger
            .open(submission_with_id("sub-2"), "hub", Epoch(4))
            .unwrap();
        let encoded = serde_json::to_string(&ledger).unwrap();
        let decoded: ModerationLedger = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ledger);
        assert_eq!(decoded.history(&id).len(), 3);
        assert_eq!(decoded.events().len(), 4);
    }

    #[test]
    fn every_state_has_an_explicit_successor_set_and_only_withdrawn_is_terminal() {
        use ModerationState::*;
        for state in [
            Submitted,
            UnderReview,
            Accepted,
            Rejected,
            Superseded,
            Withdrawn,
        ] {
            assert_eq!(
                state.successors().is_empty(),
                state.is_terminal(),
                "{state}"
            );
            assert!(
                !state.permits(state),
                "{state} must not transition to itself"
            );
        }
    }
}
