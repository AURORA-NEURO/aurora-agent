//! The published object: a BioAtlas card.
//!
//! Blueprint 34.15 and 34.16 both carry the same API object — `resource_type: "bioatlas-card"`,
//! with scope, provenance, access, health and links — and the same failure-state requirement:
//!
//! > The surface must explicitly support unavailable, controlled, stale, under-review, disputed,
//! > withdrawn, non-reproducible, and not-comparable states. It must not replace them with zero,
//! > empty, or hidden values.
//!
//! That last sentence is the whole design of this module. [`ScoreDisplay`] has no numeric variant
//! for the non-publishable cases, so a renderer cannot fall back to `0.0` or an empty cell without
//! the type changing shape. [`PublicationState`] is derived from the moderation record rather than
//! set by the publisher, so a card cannot claim to be `available` while its submission is under
//! dispute.
//!
//! # What is not implemented
//!
//! No web UI, no HTML, no templating, no `links` resolution to worlds, cells or oracles, and no
//! `health.checks` runner. This is the contract a renderer must satisfy, and the projection that
//! guarantees the renderer is given states rather than blanks.

use crate::attribution::{AccessTier, Attribution};
use crate::disclosure::HeadlineLabel;
use crate::error::HubError;
use crate::id::SubmissionId;
use crate::leaderboard::{Score, UnrankableReason};
use crate::moderation::{EventKind, ModerationRecord, ModerationState};
use crate::submission::{DeclaredScope, NonClaim, VerificationStatus};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The public state of a card. Exactly 34.15's enumeration plus `available`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PublicationState {
    /// Published, current, and rankable.
    Available,
    /// Exists but is not published: submitted and not yet reviewed, or rejected.
    Unavailable,
    /// Published, but the underlying data is not public.
    Controlled,
    /// Superseded by a successor. The prior version is preserved, not deleted.
    Stale,
    UnderReview,
    /// Was published, then returned to review. Distinct from `under-review`, because a first
    /// review and a contested result are not the same claim about the artifact.
    Disputed,
    Withdrawn,
    /// A reproduction attempt failed and an attestation was revoked.
    NonReproducible,
    /// Exists and is valid, but cannot be ordered against the board it was requested for.
    NotComparable,
}

impl PublicationState {
    pub fn as_str(self) -> &'static str {
        match self {
            PublicationState::Available => "available",
            PublicationState::Unavailable => "unavailable",
            PublicationState::Controlled => "controlled",
            PublicationState::Stale => "stale",
            PublicationState::UnderReview => "under-review",
            PublicationState::Disputed => "disputed",
            PublicationState::Withdrawn => "withdrawn",
            PublicationState::NonReproducible => "non-reproducible",
            PublicationState::NotComparable => "not-comparable",
        }
    }

    /// Only an available card may carry a number.
    pub fn may_carry_score(self) -> bool {
        self == PublicationState::Available
    }

    /// Derive the state from the moderation record, in precedence order.
    ///
    /// Withdrawal wins over everything. A failed reproduction beats the ordinary states because it
    /// is the fact a reader most needs and the one a publisher is least likely to volunteer. Then
    /// supersession, then dispute, then the plain mapping of the moderation state. A controlled
    /// access tier downgrades `available` to `controlled` last, since it describes reachability
    /// rather than standing.
    pub fn of(record: &ModerationRecord) -> PublicationState {
        if record.state == ModerationState::Withdrawn {
            return PublicationState::Withdrawn;
        }
        let attestation_revoked = record.history.iter().any(|e| {
            matches!(
                e.kind,
                EventKind::Attestation {
                    to: VerificationStatus::SelfReported,
                    ..
                }
            ) && e.reason.is_some()
        });
        if attestation_revoked {
            return PublicationState::NonReproducible;
        }
        if record.state == ModerationState::Superseded {
            return PublicationState::Stale;
        }
        if record.state == ModerationState::UnderReview {
            let was_published = record.history.iter().any(|e| {
                matches!(
                    e.kind,
                    EventKind::Transition {
                        to: ModerationState::Accepted,
                        ..
                    }
                )
            });
            return if was_published {
                PublicationState::Disputed
            } else {
                PublicationState::UnderReview
            };
        }
        match record.state {
            ModerationState::Accepted => {
                if record.submission.licence_stack.effective().access == AccessTier::Public {
                    PublicationState::Available
                } else {
                    PublicationState::Controlled
                }
            }
            _ => PublicationState::Unavailable,
        }
    }
}

impl fmt::Display for PublicationState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a card shows where a score would go.
///
/// There is deliberately no `Option<Score>`: an absent option is exactly the empty value 34.15
/// forbids, and every renderer eventually prints it as a blank cell. A withheld score carries the
/// state that withheld it and the sentence explaining why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "display")]
pub enum ScoreDisplay {
    Published {
        score: Score,
        label: HeadlineLabel,
    },
    Withheld {
        state: PublicationState,
        why: String,
    },
}

impl ScoreDisplay {
    /// The number, if there is one. Returns `None` rather than a sentinel; callers that want to
    /// render must match on the variant and print the reason.
    pub fn value(&self) -> Option<f64> {
        match self {
            ScoreDisplay::Published { score, .. } => Some(score.value),
            ScoreDisplay::Withheld { .. } => None,
        }
    }

    /// The text a public page must show in place of a number.
    pub fn withheld_notice(&self) -> Option<&str> {
        match self {
            ScoreDisplay::Withheld { why, .. } => Some(why),
            ScoreDisplay::Published { .. } => None,
        }
    }
}

/// The `bioatlas-card` object of 34.15/34.16, projected from a moderation record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BioAtlasCard {
    pub resource_type: String,
    pub resource_id: ContentHash,
    pub version: String,
    pub submission: SubmissionId,
    pub scope: DeclaredScope,
    /// Ancestor digests, not ancestor content. A card is a pointer, not a copy.
    pub provenance: Vec<ContentHash>,
    pub access: AccessTier,
    pub state: PublicationState,
    pub verification: VerificationStatus,
    pub score: ScoreDisplay,
    pub non_claims: Vec<NonClaim>,
    pub attributions: Vec<Attribution>,
    /// The limitations card of 34.15, rendered from the submission.
    pub limitations: String,
}

/// The `resource_type` constant of the blueprint API object.
pub const RESOURCE_TYPE: &str = "bioatlas-card";

impl BioAtlasCard {
    /// Project a record into a card with its score withheld.
    ///
    /// Every card starts here. Attaching a score is a separate, fallible step, so the default
    /// rendering of anything is the honest one.
    pub fn render(record: &ModerationRecord, version: impl Into<String>) -> BioAtlasCard {
        let state = PublicationState::of(record);
        let submission = &record.submission;
        BioAtlasCard {
            resource_type: RESOURCE_TYPE.to_string(),
            resource_id: submission.content.clone(),
            version: version.into(),
            submission: submission.id.clone(),
            scope: submission.scope.clone(),
            provenance: submission
                .licence_stack
                .ancestors()
                .iter()
                .map(|a| a.content.clone())
                .collect(),
            access: submission.licence_stack.effective().access,
            state,
            verification: record.verification,
            score: ScoreDisplay::Withheld {
                state,
                why: withheld_notice(state, record),
            },
            non_claims: submission.does_not_establish.clone(),
            attributions: submission.licence_stack.attributions().to_vec(),
            limitations: submission.limitations_card(),
        }
    }

    /// Attach a score, or refuse.
    ///
    /// Refuses with [`HubError::ScoreOnNonPublishableCard`] for every state except
    /// [`PublicationState::Available`]. This is the check that keeps a withdrawn or disputed
    /// artifact from carrying a live number on a public page.
    pub fn with_score(
        mut self,
        score: Score,
        label: HeadlineLabel,
    ) -> Result<BioAtlasCard, HubError> {
        if !self.state.may_carry_score() {
            return Err(HubError::ScoreOnNonPublishableCard {
                state: self.state.as_str(),
            });
        }
        self.score = ScoreDisplay::Published { score, label };
        Ok(self)
    }

    /// Re-state the card as not-comparable for a particular board, withholding its score.
    ///
    /// Used when an entry is valid and published but cannot be ordered against the board a reader
    /// asked for. 34.15 names `not-comparable` as a state precisely so that this case has somewhere
    /// to go other than a blank row.
    pub fn as_not_comparable(mut self, reason: &UnrankableReason) -> BioAtlasCard {
        self.state = PublicationState::NotComparable;
        self.score = ScoreDisplay::Withheld {
            state: PublicationState::NotComparable,
            why: describe_unrankable(reason),
        };
        self
    }
}

fn withheld_notice(state: PublicationState, record: &ModerationRecord) -> String {
    let reason = record.current_reason().unwrap_or("no reason recorded");
    match state {
        PublicationState::Available | PublicationState::Controlled => {
            "No score has been attached to this card.".to_string()
        }
        PublicationState::Withdrawn => format!(
            "Withdrawn: {reason}. The entry is retained as a tombstone; its score is not published."
        ),
        PublicationState::NonReproducible => {
            "An independent reproduction failed and the attestation was revoked. No score is \
             published."
                .to_string()
        }
        PublicationState::Stale => {
            "Superseded by a later submission. The prior version is preserved; its score is not \
             headlined."
                .to_string()
        }
        PublicationState::Disputed => format!("Returned to review after publication: {reason}."),
        PublicationState::UnderReview => "Awaiting first review.".to_string(),
        PublicationState::Unavailable => format!("Not published: {reason}."),
        PublicationState::NotComparable => {
            "Not comparable to the requested board.".to_string()
        }
    }
}

fn describe_unrankable(reason: &UnrankableReason) -> String {
    match reason {
        UnrankableReason::NotComparable { differences } => {
            let dims: Vec<&str> = differences.iter().map(|d| d.dimension.as_str()).collect();
            format!(
                "Evaluated under different conditions ({}); it is not ordered against this board.",
                dims.join(", ")
            )
        }
        UnrankableReason::NotPublished { state } => match state {
            Some(state) => format!("Not currently published (state: {state})."),
            None => "Not on the moderation ledger.".to_string(),
        },
        UnrankableReason::BelowVerificationFloor { has, floor } => format!(
            "Verification is `{has}`; this board ranks `{floor}` and above."
        ),
        UnrankableReason::Ineligible { detail } => format!("Ineligible: {detail}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::accepted_submission;
    use crate::id::Epoch;
    use crate::leaderboard::ConditionDifference;
    use crate::moderation::{Decision, ModerationLedger};

    fn ledger_at(state: ModerationState) -> ModerationLedger {
        let submission = accepted_submission();
        let id = submission.id.clone();
        let mut ledger = ModerationLedger::new();
        ledger.open(submission, "hub", Epoch(1)).unwrap();
        if state == ModerationState::Submitted {
            return ledger;
        }
        ledger
            .transition(&id, ModerationState::UnderReview, Decision::by("rev", Epoch(2)))
            .unwrap();
        if state == ModerationState::UnderReview {
            return ledger;
        }
        ledger
            .transition(&id, ModerationState::Accepted, Decision::by("rev", Epoch(3)))
            .unwrap();
        match state {
            ModerationState::Accepted => {}
            ModerationState::Withdrawn => {
                ledger
                    .transition(
                        &id,
                        ModerationState::Withdrawn,
                        Decision::by("lab-a", Epoch(4)).because("consent revoked"),
                    )
                    .unwrap();
            }
            ModerationState::Superseded => {
                ledger
                    .transition(
                        &id,
                        ModerationState::Superseded,
                        Decision::by("hub", Epoch(4))
                            .superseded_by(SubmissionId::parse("sub-2").unwrap()),
                    )
                    .unwrap();
            }
            other => panic!("fixture does not build {other}"),
        }
        ledger
    }

    fn card_for(state: ModerationState) -> BioAtlasCard {
        let ledger = ledger_at(state);
        let id = SubmissionId::parse("sub-1").unwrap();
        BioAtlasCard::render(ledger.record(&id).unwrap(), "0.1.0")
    }

    #[test]
    fn a_card_starts_with_its_score_withheld_even_when_available() {
        let card = card_for(ModerationState::Accepted);
        assert_eq!(card.state, PublicationState::Available);
        assert_eq!(card.score.value(), None);
        assert_eq!(card.resource_type, RESOURCE_TYPE);
    }

    #[test]
    fn a_withdrawn_card_cannot_be_given_a_score() {
        let card = card_for(ModerationState::Withdrawn);
        assert_eq!(card.state, PublicationState::Withdrawn);
        let err = card
            .with_score(Score::point(0.9), HeadlineLabel::HeldOut)
            .expect_err("withdrawn card");
        assert_eq!(
            err,
            HubError::ScoreOnNonPublishableCard {
                state: "withdrawn"
            }
        );
    }

    #[test]
    fn a_withheld_score_is_a_reason_and_never_a_zero() {
        for state in [
            ModerationState::Submitted,
            ModerationState::UnderReview,
            ModerationState::Withdrawn,
            ModerationState::Superseded,
        ] {
            let card = card_for(state);
            assert_eq!(card.score.value(), None, "{state}");
            let notice = card.score.withheld_notice().expect("notice present");
            assert!(notice.len() > 15, "{state}: {notice}");
        }
    }

    #[test]
    fn a_republished_submission_reads_as_disputed_not_as_first_review() {
        let mut ledger = ledger_at(ModerationState::Accepted);
        let id = SubmissionId::parse("sub-1").unwrap();
        ledger
            .transition(
                &id,
                ModerationState::UnderReview,
                Decision::by("board", Epoch(5)).because("third party reports a divergent rebuild"),
            )
            .unwrap();
        let card = BioAtlasCard::render(ledger.record(&id).unwrap(), "0.1.1");
        assert_eq!(card.state, PublicationState::Disputed);
        assert!(card
            .score
            .withheld_notice()
            .unwrap()
            .contains("divergent rebuild"));
    }

    #[test]
    fn a_revoked_attestation_makes_the_card_non_reproducible() {
        let mut ledger = ledger_at(ModerationState::Accepted);
        let id = SubmissionId::parse("sub-1").unwrap();
        ledger
            .attest(&id, VerificationStatus::Reproduced, "rev-2", Epoch(4))
            .unwrap();
        ledger
            .revoke_attestation(&id, "rev-3", Epoch(5), "rebuild produced different numbers")
            .unwrap();
        let card = BioAtlasCard::render(ledger.record(&id).unwrap(), "0.1.2");
        assert_eq!(card.state, PublicationState::NonReproducible);
        assert!(card
            .with_score(Score::point(0.1), HeadlineLabel::HeldOut)
            .is_err());
    }

    #[test]
    fn a_superseded_submission_becomes_stale_rather_than_disappearing() {
        let card = card_for(ModerationState::Superseded);
        assert_eq!(card.state, PublicationState::Stale);
        assert!(card.score.withheld_notice().unwrap().contains("preserved"));
    }

    #[test]
    fn an_available_card_accepts_a_score_with_its_disclosure_label() {
        let card = card_for(ModerationState::Accepted)
            .with_score(
                Score::with_interval(0.12, 0.09, 0.15),
                HeadlineLabel::DisclosedPack {
                    disclosed_at: Epoch(4),
                },
            )
            .expect("available card");
        assert_eq!(card.score.value(), Some(0.12));
        match &card.score {
            ScoreDisplay::Published { label, .. } => {
                assert!(label.caveat().contains("not evidence of generalisation"))
            }
            other => panic!("expected Published, got {other:?}"),
        }
    }

    #[test]
    fn a_not_comparable_card_names_the_differing_dimensions() {
        let reason = UnrankableReason::NotComparable {
            differences: vec![ConditionDifference {
                dimension: "split".into(),
                left: "hidden-holdout".into(),
                right: "public".into(),
            }],
        };
        let card = card_for(ModerationState::Accepted).as_not_comparable(&reason);
        assert_eq!(card.state, PublicationState::NotComparable);
        assert_eq!(card.score.value(), None);
        assert!(card.score.withheld_notice().unwrap().contains("split"));
    }

    #[test]
    fn a_card_carries_the_nonclaims_and_limitations_of_its_submission() {
        let card = card_for(ModerationState::Accepted);
        assert!(!card.non_claims.is_empty());
        assert!(card.limitations.contains("does not establish"));
    }

    #[test]
    fn a_card_round_trips_through_json() {
        let card = card_for(ModerationState::Withdrawn);
        let encoded = serde_json::to_string(&card).unwrap();
        let decoded: BioAtlasCard = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, card);
    }
}
