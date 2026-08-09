//! Contamination and reuse: what happens to a benchmark after people can see it.
//!
//! Blueprint 34.16 lists "contamination incidence" as a product metric and "hidden parent
//! families" and "blind scoring" as capabilities, and 34.15 requires that "hidden-test artifacts
//! never appear in model-visible logs". This module models the one thing those controls cannot
//! prevent: a public benchmark leaks, and the leak is usually caused by the hub itself publishing
//! results against it.
//!
//! # Disclosure is a ratchet
//!
//! [`DisclosureState`] moves in exactly one direction:
//!
//! ```text
//! Unknown  ->  HeldOut  ->  Disclosed  ->  Contaminated
//! ```
//!
//! Moving back is [`HubError::DisclosureRegression`]. There is no operation that "clears" a
//! contamination report, because the only honest response to a leak is a new held-out split, which
//! is a different pack with a different digest.
//!
//! # Unknown is not held-out
//!
//! The default for a pack the hub has never heard of is [`DisclosureState::Unknown`], and a
//! headline score on an `Unknown` pack is refused. This is the deliberate awkward case: it would
//! be much more convenient to treat "no leak report" as "held out". But a leak report is evidence
//! that someone looked, and no hub can distinguish a pack nobody has leaked from a pack nobody has
//! checked. The refusal names that gap instead of resolving it in the flattering direction.
//!
//! # Disclosure has a date, so old scores survive it
//!
//! A score computed before the disclosure epoch was computed against a genuinely hidden pack and
//! stays headline-eligible, labelled with the fact that the pack is now public. A score computed
//! after must acknowledge the disclosure or be refused. This is the only part of the model that
//! needs an ordering, and it is why [`Epoch`] exists.
//!
//! # What is not implemented
//!
//! No leak detection. The hub cannot look at a model's training corpus, crawl for republished
//! instances, or measure memorisation. [`ContaminationWitness`] records a finding someone else
//! made; this module decides what follows from it. The one finding it knows how to read directly
//! is the split-integrity verdict of 43.41 — see [`DisclosureLedger::record_split_integrity`],
//! which is careful about what a *passing* verdict does and does not license.

use crate::error::HubError;
use crate::id::Epoch;
use bioprism_ids::ContentHash;
use bioprism_section::{OracleStatus, OracleVerdict};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// How a pack came to be compromised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContaminationKind {
    /// Instances themselves are now public.
    InstancesPublished,
    /// Reference answers or oracle verdicts are now public.
    SolutionsPublished,
    /// The pack was found inside an evaluated system's training corpus.
    TrainingCorpusOverlap,
    /// The submitter authored or curated the pack they are being scored on.
    SubmitterAuthoredPack,
    /// The grader itself leaked signal — error messages, timing, retry behaviour.
    GraderLeak,
    /// The split-integrity oracle of 43.41 returned `invalid` on the pack. The leak is in how the
    /// pack was built rather than in who has seen it, but the consequence for a headline score is
    /// identical: the number does not mean what a reader would take it to mean.
    SplitIntegrityFailure,
}

impl ContaminationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ContaminationKind::InstancesPublished => "instances-published",
            ContaminationKind::SolutionsPublished => "solutions-published",
            ContaminationKind::TrainingCorpusOverlap => "training-corpus-overlap",
            ContaminationKind::SubmitterAuthoredPack => "submitter-authored-pack",
            ContaminationKind::GraderLeak => "grader-leak",
            ContaminationKind::SplitIntegrityFailure => "split-integrity-failure",
        }
    }
}

impl fmt::Display for ContaminationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A recorded finding that a pack is compromised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContaminationWitness {
    pub kind: ContaminationKind,
    /// What was actually observed. Not a severity score: a number here would invite a threshold,
    /// and a threshold would invite scoring on the wrong side of it.
    pub detail: String,
    pub observed_at: Epoch,
    pub reported_by: String,
}

/// What the hub knows about a pack's exposure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "disclosure")]
pub enum DisclosureState {
    /// Never recorded. Not a synonym for held-out; see the module note.
    Unknown,
    /// The operator asserts the pack has not been published.
    HeldOut,
    /// The pack became public at this epoch. Scores computed earlier remain eligible.
    Disclosed { since: Epoch },
    /// The pack is compromised. No headline score may be published on it.
    Contaminated { witness: ContaminationWitness },
}

impl DisclosureState {
    pub fn as_str(&self) -> &'static str {
        match self {
            DisclosureState::Unknown => "unknown",
            DisclosureState::HeldOut => "held-out",
            DisclosureState::Disclosed { .. } => "disclosed",
            DisclosureState::Contaminated { .. } => "contaminated",
        }
    }

    /// Position on the ratchet. Higher means more exposed.
    fn rung(&self) -> u8 {
        match self {
            DisclosureState::Unknown => 0,
            DisclosureState::HeldOut => 1,
            DisclosureState::Disclosed { .. } => 2,
            DisclosureState::Contaminated { .. } => 3,
        }
    }
}

impl fmt::Display for DisclosureState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The label a publishable headline score must carry.
///
/// Never a bare number: every eligible outcome comes with the sentence that qualifies it, so a
/// renderer cannot show the score and drop the caveat without deleting a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "label")]
pub enum HeadlineLabel {
    /// Pack was held out when the score was computed and still is.
    HeldOut,
    /// Pack is public now, but this score predates the disclosure.
    ComputedBeforeDisclosure { disclosed_at: Epoch },
    /// Pack was already public when this score was computed, and the entry says so.
    DisclosedPack { disclosed_at: Epoch },
}

impl HeadlineLabel {
    /// The caveat a public page must render next to the score.
    pub fn caveat(&self) -> String {
        match self {
            HeadlineLabel::HeldOut => {
                "Scored on a held-out pack. Held-out status is asserted by the hub operator, not \
                 proven."
                    .to_string()
            }
            HeadlineLabel::ComputedBeforeDisclosure { disclosed_at } => format!(
                "Scored before this pack was disclosed at epoch {disclosed_at}. Later scores on \
                 this pack are not comparable to it."
            ),
            HeadlineLabel::DisclosedPack { disclosed_at } => format!(
                "Scored on a pack public since epoch {disclosed_at}. This measures performance on \
                 a visible benchmark and is not evidence of generalisation."
            ),
        }
    }
}

/// Disclosure state per pack digest.
///
/// Keyed by content digest rather than by name, because "the pack" that leaked and "the pack" that
/// was scored are only the same thing if their bytes are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisclosureLedger {
    packs: BTreeMap<ContentHash, DisclosureState>,
}

impl DisclosureLedger {
    pub fn new() -> DisclosureLedger {
        DisclosureLedger::default()
    }

    pub fn state(&self, pack: &ContentHash) -> DisclosureState {
        self.packs
            .get(pack)
            .cloned()
            .unwrap_or(DisclosureState::Unknown)
    }

    pub fn entries(&self) -> impl Iterator<Item = (&ContentHash, &DisclosureState)> {
        self.packs.iter()
    }

    fn ratchet(&mut self, pack: &ContentHash, to: DisclosureState) -> Result<(), HubError> {
        let from = self.state(pack);
        if to.rung() < from.rung() {
            return Err(HubError::DisclosureRegression {
                pack: pack.to_string(),
                from: from.as_str(),
                to: to.as_str(),
            });
        }
        self.packs.insert(pack.clone(), to);
        Ok(())
    }

    /// Assert that a pack has not been published. Legal only from [`DisclosureState::Unknown`].
    pub fn declare_held_out(&mut self, pack: &ContentHash) -> Result<(), HubError> {
        self.ratchet(pack, DisclosureState::HeldOut)
    }

    /// Record that the pack is now public.
    ///
    /// The hub calls this on itself: publishing per-instance outputs against a held-out pack is a
    /// disclosure, whoever performed it. Re-disclosing an already-disclosed pack keeps the earlier
    /// epoch, because the first disclosure is the one that matters for eligibility.
    pub fn disclose(&mut self, pack: &ContentHash, at: Epoch) -> Result<(), HubError> {
        if let DisclosureState::Disclosed { since } = self.state(pack) {
            return self.ratchet(pack, DisclosureState::Disclosed { since });
        }
        self.ratchet(pack, DisclosureState::Disclosed { since: at })
    }

    /// Record a contamination finding. Terminal for this digest.
    pub fn record_contamination(
        &mut self,
        pack: &ContentHash,
        witness: ContaminationWitness,
    ) -> Result<(), HubError> {
        self.ratchet(pack, DisclosureState::Contaminated { witness })
    }

    /// Fold a split-integrity verdict (43.41, `bioprism_section::OracleVerdict`) into the ledger.
    ///
    /// Three outcomes, and the two boring ones are the ones worth being careful about:
    ///
    /// - [`OracleStatus::Invalid`] contaminates the pack, carrying the witness kinds into the
    ///   detail so the finding stays checkable rather than becoming the word "contaminated".
    /// - [`OracleStatus::Underdetermined`] changes nothing. An oracle that abstained reported that
    ///   it could not decide; converting an abstention into a finding, in either direction, is the
    ///   dishonest move 43.28 makes abstention representable in order to avoid.
    /// - [`OracleStatus::Valid`] also changes nothing, and this is the important one. A valid
    ///   split-integrity verdict says the split was drawn without leakage. It says nothing about
    ///   whether anyone has seen the pack, so it must not promote `Unknown` to `HeldOut`. Those
    ///   are different claims with different evidence, and only one of them an oracle can supply.
    ///
    /// Returns the resulting state.
    pub fn record_split_integrity(
        &mut self,
        pack: &ContentHash,
        verdict: &OracleVerdict,
        at: Epoch,
        reported_by: impl Into<String>,
    ) -> Result<DisclosureState, HubError> {
        if verdict.status == OracleStatus::Invalid {
            let kinds = verdict.witness_kinds();
            let detail = format!(
                "oracle `{}` returned invalid with {} witness(es): {}",
                verdict.oracle_kind,
                kinds.len(),
                kinds.join(", ")
            );
            self.record_contamination(
                pack,
                ContaminationWitness {
                    kind: ContaminationKind::SplitIntegrityFailure,
                    detail,
                    observed_at: at,
                    reported_by: reported_by.into(),
                },
            )?;
        }
        Ok(self.state(pack))
    }

    /// Decide whether a score may be published as a headline number, and under what label.
    ///
    /// `computed_at` is when the score was produced; `acknowledges_disclosure` is the entry's own
    /// statement that it knows the pack is public. The acknowledgement is required rather than
    /// inferred so that the dishonest case — quietly scoring a visible benchmark and presenting it
    /// as a held-out result — has to be an explicit act.
    pub fn headline_eligibility(
        &self,
        pack: &ContentHash,
        computed_at: Epoch,
        acknowledges_disclosure: bool,
    ) -> Result<HeadlineLabel, HubError> {
        match self.state(pack) {
            DisclosureState::Unknown => Err(HubError::DisclosureUnrecorded {
                pack: pack.to_string(),
            }),
            DisclosureState::Contaminated { witness } => Err(HubError::ContaminatedPack {
                pack: pack.to_string(),
                kind: witness.kind.as_str(),
                detail: witness.detail,
            }),
            DisclosureState::HeldOut => Ok(HeadlineLabel::HeldOut),
            DisclosureState::Disclosed { since } => {
                if computed_at < since {
                    Ok(HeadlineLabel::ComputedBeforeDisclosure { disclosed_at: since })
                } else if acknowledges_disclosure {
                    Ok(HeadlineLabel::DisclosedPack { disclosed_at: since })
                } else {
                    Err(HubError::UnacknowledgedDisclosure {
                        pack: pack.to_string(),
                        disclosed_at: since.get(),
                    })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> ContentHash {
        ContentHash::of_bytes(b"holdout-pack-v3")
    }

    fn witness() -> ContaminationWitness {
        ContaminationWitness {
            kind: ContaminationKind::TrainingCorpusOverlap,
            detail: "412 of 500 instances found verbatim in an evaluated system's corpus".into(),
            observed_at: Epoch(9),
            reported_by: "audit-2".into(),
        }
    }

    #[test]
    fn an_unrecorded_pack_is_unknown_rather_than_held_out() {
        let ledger = DisclosureLedger::new();
        assert_eq!(ledger.state(&pack()), DisclosureState::Unknown);
        let err = ledger
            .headline_eligibility(&pack(), Epoch(1), false)
            .expect_err("no disclosure record");
        assert_eq!(
            err,
            HubError::DisclosureUnrecorded {
                pack: pack().to_string()
            }
        );
    }

    #[test]
    fn a_headline_score_on_a_contaminated_pack_is_refused() {
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        ledger.record_contamination(&pack(), witness()).unwrap();
        let err = ledger
            .headline_eligibility(&pack(), Epoch(1), true)
            .expect_err("contaminated pack");
        assert_eq!(
            err,
            HubError::ContaminatedPack {
                pack: pack().to_string(),
                kind: "training-corpus-overlap",
                detail: "412 of 500 instances found verbatim in an evaluated system's corpus"
                    .into(),
            }
        );
    }

    #[test]
    fn contamination_cannot_be_walked_back_to_held_out() {
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        ledger.record_contamination(&pack(), witness()).unwrap();
        let err = ledger
            .declare_held_out(&pack())
            .expect_err("contamination is a ratchet");
        assert_eq!(
            err,
            HubError::DisclosureRegression {
                pack: pack().to_string(),
                from: "contaminated",
                to: "held-out",
            }
        );
    }

    #[test]
    fn a_disclosed_pack_cannot_be_declared_held_out_again() {
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        ledger.disclose(&pack(), Epoch(5)).unwrap();
        let err = ledger.declare_held_out(&pack()).expect_err("already disclosed");
        assert!(matches!(err, HubError::DisclosureRegression { from: "disclosed", .. }));
    }

    #[test]
    fn a_score_computed_before_disclosure_stays_eligible_and_one_after_does_not() {
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        ledger.disclose(&pack(), Epoch(5)).unwrap();

        let before = ledger.headline_eligibility(&pack(), Epoch(4), false).unwrap();
        assert_eq!(
            before,
            HeadlineLabel::ComputedBeforeDisclosure {
                disclosed_at: Epoch(5)
            }
        );

        let err = ledger
            .headline_eligibility(&pack(), Epoch(6), false)
            .expect_err("post-disclosure score with no acknowledgement");
        assert_eq!(
            err,
            HubError::UnacknowledgedDisclosure {
                pack: pack().to_string(),
                disclosed_at: 5,
            }
        );

        let acknowledged = ledger.headline_eligibility(&pack(), Epoch(6), true).unwrap();
        assert_eq!(
            acknowledged,
            HeadlineLabel::DisclosedPack {
                disclosed_at: Epoch(5)
            }
        );
    }

    #[test]
    fn re_disclosure_keeps_the_first_disclosure_epoch() {
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        ledger.disclose(&pack(), Epoch(5)).unwrap();
        ledger.disclose(&pack(), Epoch(11)).unwrap();
        assert_eq!(
            ledger.state(&pack()),
            DisclosureState::Disclosed { since: Epoch(5) }
        );
    }

    #[test]
    fn every_eligible_label_carries_a_caveat_naming_what_it_does_not_show() {
        let labels = [
            HeadlineLabel::HeldOut,
            HeadlineLabel::ComputedBeforeDisclosure {
                disclosed_at: Epoch(5),
            },
            HeadlineLabel::DisclosedPack {
                disclosed_at: Epoch(5),
            },
        ];
        for label in labels {
            let caveat = label.caveat();
            assert!(caveat.len() > 40, "caveat too thin: {caveat}");
            assert!(caveat.ends_with('.'), "caveat must be a sentence: {caveat}");
        }
    }

    #[test]
    fn an_invalid_split_integrity_verdict_contaminates_the_pack_and_names_its_witnesses() {
        use bioprism_section::LeakageWitness;
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        let verdict = OracleVerdict::new(
            "split-integrity/0.1",
            vec![LeakageWitness::IdentityLeakage {
                alias: "ALT-77".into(),
                subjects: vec!["S001".into(), "S003".into()],
                splits: vec!["train".into(), "holdout".into()],
            }],
        );
        let state = ledger
            .record_split_integrity(&pack(), &verdict, Epoch(4), "oracle-runner")
            .unwrap();
        assert!(matches!(state, DisclosureState::Contaminated { .. }));

        let err = ledger
            .headline_eligibility(&pack(), Epoch(1), true)
            .expect_err("split integrity failed");
        match err {
            HubError::ContaminatedPack { kind, detail, .. } => {
                assert_eq!(kind, "split-integrity-failure");
                assert!(detail.contains("identity_leakage"), "{detail}");
            }
            other => panic!("expected ContaminatedPack, got {other:?}"),
        }
    }

    #[test]
    fn a_valid_split_integrity_verdict_does_not_make_an_unknown_pack_held_out() {
        let mut ledger = DisclosureLedger::new();
        let verdict = OracleVerdict::new("split-integrity/0.1", Vec::new());
        let state = ledger
            .record_split_integrity(&pack(), &verdict, Epoch(4), "oracle-runner")
            .unwrap();
        assert_eq!(state, DisclosureState::Unknown, "a clean split is not a secret split");
        assert!(ledger.headline_eligibility(&pack(), Epoch(5), true).is_err());
    }

    #[test]
    fn an_abstaining_split_integrity_oracle_neither_contaminates_nor_certifies() {
        use bioprism_section::LeakageWitness;
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        let verdict = OracleVerdict::abstain(
            "split-integrity/0.1",
            vec![LeakageWitness::PreprocessingLeakage {
                detail: "normalisation provenance unavailable".into(),
            }],
        );
        let state = ledger
            .record_split_integrity(&pack(), &verdict, Epoch(4), "oracle-runner")
            .unwrap();
        assert_eq!(state, DisclosureState::HeldOut);
        assert_eq!(
            ledger.headline_eligibility(&pack(), Epoch(5), false).unwrap(),
            HeadlineLabel::HeldOut
        );
    }

    #[test]
    fn the_disclosure_ledger_round_trips_through_json() {
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&pack()).unwrap();
        let other = ContentHash::of_bytes(b"public-pack");
        ledger.disclose(&other, Epoch(2)).unwrap();
        let encoded = serde_json::to_string(&ledger).unwrap();
        let decoded: DisclosureLedger = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ledger);
    }
}
