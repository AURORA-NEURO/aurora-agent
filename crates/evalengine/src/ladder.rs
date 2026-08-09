//! The scoring ladder: `deterministic > execution > property > statistical > judge`.
//!
//! Blueprint 07.01 states the rule as "lower-priority evidence can add nuance but cannot silently
//! override a higher-priority contradiction", and raises it to an invariant: "nondeterministic
//! judgements never silently override deterministic or execution-grounded evidence."
//!
//! # Relationship to the oracle's evidence ladder
//!
//! `bioprism-oracle`'s `EvidenceTier` ranks **judgements about whether a claim is true** and its
//! operation is `may_override`, answering which of two contradictory judgements stands. This
//! module ranks **scores about what a run achieved** and its operation is [`compose`], answering
//! which tier's conclusion the report carries. The rungs are the same rungs — there is one
//! evidence hierarchy in this system, not two — but the demotion rules, circularity handling and
//! verdict arithmetic of the oracle are deliberately not duplicated here. This crate never
//! produces a judgement; it consumes [`Contribution`]s that already carry one.
//!
//! The tier is repeated as a plain ordering rather than imported because a scorer that links the
//! oracle stack can no longer be used to score the oracle stack, and because 07.10 treats an
//! evaluator that shares machinery with what it evaluates as a security problem rather than a
//! convenience.
//!
//! # What makes the invariant structural
//!
//! [`compose`] never consults a weaker contribution when choosing the conclusion. It walks tiers
//! strongest-first, stops at the first tier that reaches one, and everything below that tier is
//! filed under [`ScoredResult::detail`]. There is no code path in which a `Judge` contribution
//! writes [`ScoredResult::conclusion`] while an `Execution` contribution exists — not a check that
//! could be forgotten, but an absence of the branch. Confidence is never read: a judge at 0.99 is
//! still a judge.
//!
//! A weaker contribution that *would have been more favourable* is not discarded either. It is
//! recorded in [`ScoredResult::suppressed_raises`], because the pattern "the deterministic checker
//! failed it and the judge liked it" is the reward-hacking signal of 07.10 and deleting it would
//! hide exactly the thing worth looking at.
//!
//! # Unknown does not fall through
//!
//! If the strongest tier concluded [`Conclusion::Unknown`], the result is unknown. A weaker tier is
//! not silently promoted to fill the gap, because substituting weaker evidence for the evidence you
//! meant to have is how a broken deterministic checker turns into a judge-scored benchmark without
//! anyone noticing. A caller who wants the fallback declares it through [`UnknownPolicy`], and the
//! declaration is carried in the output.
//!
//! # Not implemented here
//!
//! Evaluator selection by cell and claim (07.01 "select evaluators by cell and claim"), evaluator
//! health checks, and the human resolution workflow that a [`Conclusion::Disputed`] result should
//! enter. This module detects the disagreement and refuses to break the tie; routing it is out of
//! scope.

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::error::EvalError;
use crate::score::{credit_for, Conclusion, Credit, CreditPolicy, RubricProgress, Veto};

/// A rung of the scoring ladder, declared weakest-first so that derived `Ord` means "stronger
/// evidence".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreTier {
    /// A model or human applying a rubric.
    Judge,
    /// An estimate over a sample: agreement rates, calibration, effect sizes.
    Statistical,
    /// A named relation recomputed over the result: metamorphic, monotonic, conserved.
    Property,
    /// The workflow was rerun and its outputs compared within a declared tolerance.
    Execution,
    /// A machine-checkable invariant of the artifact: schema, checksum, identifier, unit.
    Deterministic,
}

impl ScoreTier {
    /// Every tier, weakest first.
    pub const ALL: [ScoreTier; 5] = [
        ScoreTier::Judge,
        ScoreTier::Statistical,
        ScoreTier::Property,
        ScoreTier::Execution,
        ScoreTier::Deterministic,
    ];

    /// The two tiers whose conclusions are grounded in the artifact or in an execution of it.
    pub fn is_grounded(self) -> bool {
        matches!(self, ScoreTier::Deterministic | ScoreTier::Execution)
    }

    /// Whether a conclusion at this tier is reproducible byte-for-byte.
    pub fn is_reproducible(self) -> bool {
        !matches!(self, ScoreTier::Judge | ScoreTier::Statistical)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ScoreTier::Judge => "judge",
            ScoreTier::Statistical => "statistical",
            ScoreTier::Property => "property",
            ScoreTier::Execution => "execution",
            ScoreTier::Deterministic => "deterministic",
        }
    }
}

impl fmt::Display for ScoreTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A pointer from a score back to what produced it.
///
/// Blueprint 07.01's evidence graph requires each score to name its source state, events,
/// artifacts, oracle version, evaluator implementation and assumptions. This crate carries the
/// handles opaquely; resolving them belongs to the store.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceRef {
    /// `artifact`, `event`, `state`, `oracle_version`, `trial`, and so on.
    pub kind: String,
    pub handle: String,
}

impl EvidenceRef {
    pub fn new(kind: impl Into<String>, handle: impl Into<String>) -> Self {
        EvidenceRef {
            kind: kind.into(),
            handle: handle.into(),
        }
    }
}

/// One evaluator's tiered statement about one result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    pub tier: ScoreTier,
    /// Implementation identity of the evaluator, version included by convention.
    pub evaluator: String,
    pub conclusion: Conclusion,
    /// Rubric weights behind a partial conclusion. Empty when the conclusion is not rubric-derived.
    #[serde(default)]
    pub progress: RubricProgress,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<EvidenceRef>,
    /// Free-text nuance. Always retained, never promoted into the conclusion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// A veto raised by this evaluator. Vetoes act regardless of tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub veto: Option<Veto>,
}

impl Contribution {
    pub fn new(tier: ScoreTier, evaluator: impl Into<String>, conclusion: Conclusion) -> Self {
        Contribution {
            tier,
            evaluator: evaluator.into(),
            conclusion,
            progress: RubricProgress::default(),
            evidence: Vec::new(),
            notes: Vec::new(),
            veto: None,
        }
    }

    pub fn with_progress(mut self, progress: RubricProgress) -> Self {
        self.progress = progress;
        self
    }

    pub fn with_evidence(mut self, evidence: EvidenceRef) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_veto(mut self, veto: Veto) -> Self {
        self.veto = Some(veto);
        self
    }
}

/// What to do when the deciding tier concluded [`Conclusion::Unknown`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownPolicy {
    /// Report unknown. The default, and the only option that adds no assumption.
    #[default]
    Block,
    /// Let the next weaker tier decide, on the record.
    FallBackToNextTier { declared_by: String },
    /// Treat the unknown as a failure, on the record.
    TreatAsFail { declared_by: String },
}

/// A weaker-tier contribution that was more favourable than the conclusion that stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuppressedRaise {
    pub tier: ScoreTier,
    pub evaluator: String,
    pub would_have_concluded: Conclusion,
    pub standing_conclusion: Conclusion,
    /// Why it did not apply, in the report's own words.
    pub reason: String,
}

/// Two evaluators at the same tier reaching different conclusions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disagreement {
    pub tier: ScoreTier,
    pub positions: Vec<(String, Conclusion)>,
}

/// A weaker contribution, retained as detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detail {
    pub tier: ScoreTier,
    pub evaluator: String,
    pub conclusion: Conclusion,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// The composed score for one result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredResult {
    pub result_id: String,
    pub conclusion: Conclusion,
    pub deciding_tier: ScoreTier,
    pub deciding_evaluators: Vec<String>,
    /// The conclusion that stood before any veto was applied. Present only when a veto fired, so
    /// that a safety stop never erases the capability reading underneath it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conclusion_before_veto: Option<Conclusion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vetoes: Vec<Veto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detail: Vec<Detail>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed_raises: Vec<SuppressedRaise>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disagreements: Vec<Disagreement>,
    /// The weakest tier that contributed anything at all, so a reader can see what the result
    /// rests on without expanding the detail list.
    pub weakest_tier_present: ScoreTier,
    pub unknown_policy: UnknownPolicy,
    /// Rubric weights from the deciding contribution, carried so credit can be recomputed under a
    /// different policy without rerunning anything.
    #[serde(default)]
    pub progress: RubricProgress,
}

impl ScoredResult {
    pub fn credit(&self, policy: &CreditPolicy) -> Credit {
        credit_for(self.conclusion, self.progress, policy)
    }

    /// Whether a human has to look at this before it can be aggregated.
    pub fn needs_resolution(&self) -> bool {
        !self.disagreements.is_empty() || self.conclusion == Conclusion::Disputed
    }

    /// Whether the standing conclusion is grounded in the artifact or in an execution of it.
    pub fn is_grounded(&self) -> bool {
        self.deciding_tier.is_grounded()
    }

    /// The reward-hacking signal: a weaker evaluator was more generous than a grounded one.
    pub fn has_optimistic_weak_evidence(&self) -> bool {
        self.suppressed_raises
            .iter()
            .any(|raise| raise.tier < self.deciding_tier)
    }
}

/// Compose tiered contributions into one scored result.
///
/// Walks the ladder strongest-first. The first tier that reaches a conclusion sets it; every
/// weaker contribution becomes detail. Same-tier contradiction is not resolved — it produces
/// [`Conclusion::Disputed`], which no aggregate in this crate will consume.
pub fn compose(
    result_id: &str,
    contributions: &[Contribution],
    policy: &UnknownPolicy,
) -> Result<ScoredResult, EvalError> {
    if contributions.is_empty() {
        return Err(EvalError::NoContributions {
            result_id: result_id.to_string(),
        });
    }

    let vetoes: Vec<Veto> = contributions
        .iter()
        .filter_map(|c| c.veto.clone())
        .collect();

    let weakest_tier_present = contributions
        .iter()
        .map(|c| c.tier)
        .min()
        .expect("non-empty");

    let mut disagreements = Vec::new();
    let mut decided: Option<(ScoreTier, Conclusion, Vec<String>, RubricProgress)> = None;

    for tier in ScoreTier::ALL.iter().rev().copied() {
        let at_tier: Vec<&Contribution> =
            contributions.iter().filter(|c| c.tier == tier).collect();
        if at_tier.is_empty() {
            continue;
        }

        let first = at_tier[0].conclusion;
        if at_tier.iter().any(|c| c.conclusion != first) {
            disagreements.push(Disagreement {
                tier,
                positions: at_tier
                    .iter()
                    .map(|c| (c.evaluator.clone(), c.conclusion))
                    .collect(),
            });
            decided = Some((
                tier,
                Conclusion::Disputed,
                at_tier.iter().map(|c| c.evaluator.clone()).collect(),
                RubricProgress::default(),
            ));
            break;
        }

        let conclusion = first;
        let evaluators: Vec<String> = at_tier.iter().map(|c| c.evaluator.clone()).collect();
        let progress = at_tier[0].progress;

        if conclusion == Conclusion::Unknown {
            match policy {
                UnknownPolicy::Block => {
                    decided = Some((tier, Conclusion::Unknown, evaluators, progress));
                    break;
                }
                UnknownPolicy::TreatAsFail { .. } => {
                    decided = Some((tier, Conclusion::Fail, evaluators, progress));
                    break;
                }
                UnknownPolicy::FallBackToNextTier { .. } => continue,
            }
        }

        decided = Some((tier, conclusion, evaluators, progress));
        break;
    }

    // Reachable only when every tier concluded unknown under a declared fallback: the fallback
    // ran out of tiers, which is still unknown and must not become a failure by exhaustion.
    let (deciding_tier, conclusion, deciding_evaluators, progress) =
        decided.unwrap_or_else(|| {
            let tier = contributions.iter().map(|c| c.tier).max().expect("non-empty");
            (
                tier,
                Conclusion::Unknown,
                contributions
                    .iter()
                    .filter(|c| c.tier == tier)
                    .map(|c| c.evaluator.clone())
                    .collect(),
                RubricProgress::default(),
            )
        });

    let mut detail = Vec::new();
    let mut suppressed_raises = Vec::new();
    for contribution in contributions {
        if contribution.tier >= deciding_tier && contribution.conclusion == conclusion {
            continue;
        }
        detail.push(Detail {
            tier: contribution.tier,
            evaluator: contribution.evaluator.clone(),
            conclusion: contribution.conclusion,
            notes: contribution.notes.clone(),
        });
        if contribution.tier < deciding_tier && is_more_favourable(contribution.conclusion, conclusion)
        {
            suppressed_raises.push(SuppressedRaise {
                tier: contribution.tier,
                evaluator: contribution.evaluator.clone(),
                would_have_concluded: contribution.conclusion,
                standing_conclusion: conclusion,
                reason: format!(
                    "{} evidence cannot raise a {} conclusion; retained as detail",
                    contribution.tier, deciding_tier
                ),
            });
        }
    }
    detail.sort_by(|a, b| b.tier.cmp(&a.tier).then(a.evaluator.cmp(&b.evaluator)));

    let (final_conclusion, conclusion_before_veto) = if vetoes.is_empty() {
        (conclusion, None)
    } else {
        (Conclusion::Vetoed, Some(conclusion))
    };

    Ok(ScoredResult {
        result_id: result_id.to_string(),
        conclusion: final_conclusion,
        deciding_tier,
        deciding_evaluators,
        conclusion_before_veto,
        vetoes,
        detail,
        suppressed_raises,
        disagreements,
        weakest_tier_present,
        unknown_policy: policy.clone(),
        progress,
    })
}

/// Whether `candidate` would read as a better result than `standing`.
///
/// Used only to label a suppressed raise. The order is a declared reporting convention, not a
/// measurement: it exists so that "the judge was more generous than the checker" is detectable,
/// and it never feeds a conclusion.
fn is_more_favourable(candidate: Conclusion, standing: Conclusion) -> bool {
    fn rank(conclusion: Conclusion) -> Option<u8> {
        match conclusion {
            Conclusion::Pass => Some(5),
            Conclusion::UnsupportedPass => Some(4),
            Conclusion::PartialCredit => Some(3),
            Conclusion::ContradictedPass => Some(2),
            Conclusion::Fail => Some(1),
            Conclusion::Vetoed => Some(0),
            Conclusion::Unknown
            | Conclusion::Disputed
            | Conclusion::Abstained
            | Conclusion::JustificationUnexamined => None,
        }
    }
    match (rank(candidate), rank(standing)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::VetoKind;

    fn judge(conclusion: Conclusion) -> Contribution {
        Contribution::new(ScoreTier::Judge, "rubric-judge@1", conclusion)
    }

    fn deterministic(conclusion: Conclusion) -> Contribution {
        Contribution::new(ScoreTier::Deterministic, "schema-check@1", conclusion)
    }

    #[test]
    fn tiers_are_ordered_so_that_greater_means_stronger_evidence() {
        assert!(ScoreTier::Deterministic > ScoreTier::Execution);
        assert!(ScoreTier::Execution > ScoreTier::Property);
        assert!(ScoreTier::Property > ScoreTier::Statistical);
        assert!(ScoreTier::Statistical > ScoreTier::Judge);
        assert!(ScoreTier::ALL.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn a_judge_cannot_raise_a_deterministic_failure() {
        let scored = compose(
            "r1",
            &[deterministic(Conclusion::Fail), judge(Conclusion::Pass)],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Fail);
        assert_eq!(scored.deciding_tier, ScoreTier::Deterministic);
    }

    #[test]
    fn a_suppressed_raise_is_reported_rather_than_discarded() {
        let scored = compose(
            "r1",
            &[
                deterministic(Conclusion::Fail),
                judge(Conclusion::Pass).with_note("the plan reads well"),
            ],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.suppressed_raises.len(), 1);
        assert_eq!(
            scored.suppressed_raises[0].would_have_concluded,
            Conclusion::Pass
        );
        assert!(scored.has_optimistic_weak_evidence());
        assert!(scored
            .detail
            .iter()
            .any(|d| d.notes.iter().any(|n| n.contains("reads well"))));
    }

    #[test]
    fn a_weaker_agreeing_contribution_adds_detail_without_changing_the_conclusion() {
        let scored = compose(
            "r1",
            &[
                deterministic(Conclusion::Pass),
                judge(Conclusion::Pass).with_note("clean trajectory"),
            ],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Pass);
        assert_eq!(scored.deciding_tier, ScoreTier::Deterministic);
        assert!(scored.suppressed_raises.is_empty());
        assert_eq!(scored.detail.len(), 1);
    }

    #[test]
    fn same_tier_contradiction_is_disputed_rather_than_broken_by_a_tiebreak() {
        let scored = compose(
            "r1",
            &[
                Contribution::new(ScoreTier::Execution, "runner-a@1", Conclusion::Pass),
                Contribution::new(ScoreTier::Execution, "runner-b@1", Conclusion::Fail),
            ],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Disputed);
        assert!(scored.needs_resolution());
        assert_eq!(scored.disagreements.len(), 1);
        assert_eq!(scored.disagreements[0].positions.len(), 2);
    }

    #[test]
    fn an_unknown_at_the_strongest_tier_does_not_fall_through_to_a_judge() {
        let scored = compose(
            "r1",
            &[deterministic(Conclusion::Unknown), judge(Conclusion::Pass)],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Unknown);
        assert_eq!(scored.deciding_tier, ScoreTier::Deterministic);
    }

    #[test]
    fn falling_back_from_an_unknown_requires_a_named_declaration_and_records_it() {
        let policy = UnknownPolicy::FallBackToNextTier {
            declared_by: "pack-owner".to_string(),
        };
        let scored = compose(
            "r1",
            &[deterministic(Conclusion::Unknown), judge(Conclusion::Pass)],
            &policy,
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Pass);
        assert_eq!(scored.deciding_tier, ScoreTier::Judge);
        assert_eq!(scored.unknown_policy, policy);
    }

    #[test]
    fn an_all_unknown_ladder_stays_unknown_even_under_a_declared_fallback() {
        let scored = compose(
            "r1",
            &[
                deterministic(Conclusion::Unknown),
                judge(Conclusion::Unknown),
            ],
            &UnknownPolicy::FallBackToNextTier {
                declared_by: "pack-owner".to_string(),
            },
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Unknown);
    }

    #[test]
    fn a_veto_at_any_tier_removes_success_but_preserves_what_stood_before_it() {
        let scored = compose(
            "r1",
            &[
                deterministic(Conclusion::Pass),
                judge(Conclusion::Pass).with_veto(Veto::new(
                    VetoKind::DataLeakage,
                    "leak-scan@2",
                    "held-out identifiers appeared in the answer",
                )),
            ],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Vetoed);
        assert_eq!(scored.conclusion_before_veto, Some(Conclusion::Pass));
        assert_eq!(scored.vetoes.len(), 1);
        assert!(!scored.credit(&CreditPolicy::default()).full_pass);
    }

    #[test]
    fn a_result_with_no_contributions_is_an_error_not_a_zero() {
        let err = compose("r1", &[], &UnknownPolicy::Block).unwrap_err();
        assert_eq!(
            err,
            EvalError::NoContributions {
                result_id: "r1".to_string()
            }
        );
    }

    #[test]
    fn the_weakest_tier_present_is_reported_so_a_reader_sees_what_the_score_rests_on() {
        let scored = compose(
            "r1",
            &[
                Contribution::new(ScoreTier::Property, "metamorphic@1", Conclusion::Pass),
                judge(Conclusion::Pass),
            ],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.weakest_tier_present, ScoreTier::Judge);
        assert!(!scored.is_grounded());
    }

    #[test]
    fn a_scored_result_round_trips_through_json() {
        let scored = compose(
            "r1",
            &[
                deterministic(Conclusion::Fail)
                    .with_evidence(EvidenceRef::new("artifact", "sha256:beef")),
                judge(Conclusion::Pass),
            ],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        let text = serde_json::to_string(&scored).expect("serialize");
        let back: ScoredResult = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(scored, back);
    }
}
