//! Disagreement, adjudication, and appeals (31.15).
//!
//! 31.15's required functions are: classify the disagreement source, check artifact and version
//! mismatches, request independent review, **preserve pre-adjudication outputs**, issue a
//! resolution or an unresolved state, and support appeals. Its evaluation metrics include
//! *dissent retention*, which only makes sense if dissent survives resolution.
//!
//! So the type here is append-only in spirit. [`Disagreement::positions`] records who held what,
//! and adjudicating never removes an entry from it — [`Resolution::Overturned`] names the position
//! that lost, and the oracles that held it are still listed beside it. A reader of an adjudicated
//! record can always reconstruct the pre-adjudication state, which is the whole point of the
//! 31.15 worked case where a benchmark score changes after a specimen swap and "the old result
//! remains auditable".
//!
//! Adjudication moves strictly *up* the ladder. An oracle at or below the disputed tier is
//! rejected with [`OracleError::AdjudicationTierTooLow`], because letting a same-tier third
//! opinion settle a two-way split is majority voting, which 40.21 rules out for combination and
//! which would be no more defensible here.
//!
//! Not implemented: the workflow half of 31.15 — review queues, resolution-time metrics, appeal
//! routing, notification of affected result bundles. Those need an event ledger and a service;
//! this crate provides the record they would move around.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::OracleError;
use crate::judgement::{Judgement, Position};
use crate::ladder::EvidenceTier;
use crate::manifest::OracleRef;
use crate::time::UtcTimestamp;

/// Why two oracles at the same tier reached different positions.
///
/// The classification is not cosmetic: it selects the settlement. A version mismatch is settled by
/// aligning versions, an independence violation by independent review, and genuine ambiguity only
/// by evidence from a stronger rung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum DisagreementSource {
    /// The same oracle identity at two versions. 31.16's territory, not a scientific dispute.
    VersionMismatch {
        id: String,
        versions: BTreeSet<String>,
    },
    /// The disagreeing oracles establish disjoint planes, so they are not answering one question.
    /// A schema oracle and a policy judge "disagreeing" is usually this.
    ScopeMismatch {
        planes: BTreeMap<String, Vec<String>>,
    },
    /// At least one party shares data, code, or assumptions with the evaluated system, so its
    /// position may be an echo rather than an observation (31.01).
    IndependenceViolation { circular: Vec<OracleRef> },
    /// Independent, same-scope, same-version oracles that genuinely differ. The only case where
    /// the disagreement is about the biology rather than about the plumbing.
    GenuineAmbiguity,
}

/// What would settle a disagreement. 31.15 requires a record of the route to resolution, not just
/// the fact of the conflict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "settlement", rename_all = "snake_case")]
pub enum Settlement {
    /// Evidence from a strictly stronger rung.
    HigherTierOracle { at_least: EvidenceTier },
    /// Aligning the parties on one reference version before asking again.
    VersionAlignment { id: String },
    /// A reviewer with no shared inputs with either party or with the evaluated system.
    IndependentReview { reason: String },
    /// The artifact itself is ambiguous or damaged; no oracle can resolve it as it stands.
    ArtifactRepair { pointer: String },
    /// Only a later observation can decide, which by 31.01 may evaluate but may not be
    /// backdated into the earlier context.
    LongitudinalObservation { awaiting: String },
}

/// The state of a disagreement record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "resolution", rename_all = "snake_case")]
pub enum Resolution {
    /// No adjudication has been applied.
    Open,
    /// A stronger oracle agreed with a position already held.
    Upheld {
        by: OracleRef,
        at: UtcTimestamp,
        position: Position,
    },
    /// A stronger oracle took a position none of the parties held. `superseded` names what lost;
    /// the parties that held it remain in [`Disagreement::positions`].
    Overturned {
        by: OracleRef,
        at: UtcTimestamp,
        position: Position,
        superseded: BTreeSet<Position>,
    },
    /// No available oracle can settle it. 31.01: unresolved is a legitimate terminal state.
    Unresolvable { reason: String },
}

impl Resolution {
    pub fn is_open(&self) -> bool {
        matches!(self, Resolution::Open)
    }
}

/// A typed record of who disagreed, at what tier, and what would settle it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Disagreement {
    /// The rung the dispute sits on. Cross-tier conflict is not a disagreement — it is an
    /// override attempt, recorded as [`crate::SuppressedOverride`].
    pub tier: EvidenceTier,
    /// Every position held, and by whom. Never pruned.
    pub positions: BTreeMap<Position, Vec<OracleRef>>,
    pub source: DisagreementSource,
    pub would_be_settled_by: Vec<Settlement>,
    pub resolution: Resolution,
}

impl Disagreement {
    /// Builds a record from the judgements that split, classifying the source and deriving the
    /// settlement route.
    ///
    /// Callers pass every judgement at the deciding tier, including abstentions; abstentions are
    /// recorded in `positions` but do not count as parties to the dispute.
    pub fn between(tier: EvidenceTier, judgements: &[Judgement]) -> Self {
        let mut positions: BTreeMap<Position, Vec<OracleRef>> = BTreeMap::new();
        for judgement in judgements {
            positions
                .entry(judgement.position)
                .or_default()
                .push(judgement.oracle.clone());
        }

        let source = classify(judgements);
        let would_be_settled_by = settlements_for(tier, &source);

        Disagreement {
            tier,
            positions,
            source,
            would_be_settled_by,
            resolution: Resolution::Open,
        }
    }

    /// The positions actually in dispute, excluding abstentions.
    pub fn contested(&self) -> BTreeSet<Position> {
        self.positions
            .keys()
            .copied()
            .filter(|position| !position.is_abstention())
            .collect()
    }

    /// Applies an adjudicating judgement.
    ///
    /// Rejects three things, each of which would otherwise turn adjudication into laundering:
    /// an adjudicator at or below the disputed tier, an adjudicator that abstained, and an
    /// inadmissible adjudicator (31.16 — an expired oracle is inadmissible everywhere, including
    /// here). On success the dispute's positions are left exactly as they were.
    pub fn adjudicate(
        mut self,
        adjudicator: &Judgement,
        at: &UtcTimestamp,
    ) -> Result<Self, OracleError> {
        if !adjudicator.is_admissible() {
            return Err(OracleError::InadmissibleAdjudicator {
                oracle: adjudicator.oracle.to_string(),
                reason: adjudicator.admissibility.reason(),
            });
        }
        if adjudicator.position.is_abstention() {
            return Err(OracleError::AdjudicationAbstains {
                oracle: adjudicator.oracle.to_string(),
            });
        }
        if adjudicator.tier <= self.tier {
            return Err(OracleError::AdjudicationTierTooLow {
                dispute: self.tier,
                offered: adjudicator.tier,
            });
        }

        let contested = self.contested();
        self.resolution = if contested.contains(&adjudicator.position) {
            Resolution::Upheld {
                by: adjudicator.oracle.clone(),
                at: at.clone(),
                position: adjudicator.position,
            }
        } else {
            Resolution::Overturned {
                by: adjudicator.oracle.clone(),
                at: at.clone(),
                position: adjudicator.position,
                superseded: contested,
            }
        };
        Ok(self)
    }

    /// Closes the record without a decision, which 31.01 treats as a legitimate outcome rather
    /// than a failure to try.
    pub fn declare_unresolvable(mut self, reason: impl Into<String>) -> Self {
        self.resolution = Resolution::Unresolvable {
            reason: reason.into(),
        };
        self
    }
}

fn classify(judgements: &[Judgement]) -> DisagreementSource {
    let contested: Vec<&Judgement> = judgements
        .iter()
        .filter(|judgement| !judgement.position.is_abstention())
        .collect();

    let mut versions_by_id: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for judgement in &contested {
        versions_by_id
            .entry(judgement.oracle.kind().to_string())
            .or_default()
            .insert(judgement.oracle.version.to_string());
    }
    if let Some((id, versions)) = versions_by_id.iter().find(|(_, v)| v.len() > 1) {
        return DisagreementSource::VersionMismatch {
            id: id.clone(),
            versions: versions.clone(),
        };
    }

    let circular: Vec<OracleRef> = contested
        .iter()
        .filter(|judgement| judgement.was_demoted())
        .map(|judgement| judgement.oracle.clone())
        .collect();
    if !circular.is_empty() {
        return DisagreementSource::IndependenceViolation { circular };
    }

    let planes: BTreeMap<String, Vec<String>> = contested
        .iter()
        .map(|judgement| {
            (
                judgement.oracle.to_string(),
                judgement
                    .establishes
                    .iter()
                    .map(|plane| plane.to_string())
                    .collect(),
            )
        })
        .collect();
    let shares_a_plane = contested.iter().enumerate().any(|(i, left)| {
        contested
            .iter()
            .skip(i + 1)
            .any(|right| !left.establishes.is_disjoint(&right.establishes))
    });
    if !shares_a_plane && contested.len() > 1 {
        return DisagreementSource::ScopeMismatch { planes };
    }

    DisagreementSource::GenuineAmbiguity
}

/// Derives the route to resolution.
///
/// Below the top rung the answer is always "ask something stronger". At the top rung there is
/// nothing stronger: two deterministic oracles contradicting each other is a defect in the
/// oracles or an ambiguity in the artifact, never a fact about biology, and never something a
/// judge may be invited to break the tie on. That case routes to artifact repair and independent
/// review instead.
fn settlements_for(tier: EvidenceTier, source: &DisagreementSource) -> Vec<Settlement> {
    let mut settlements = Vec::new();

    match source {
        DisagreementSource::VersionMismatch { id, .. } => {
            settlements.push(Settlement::VersionAlignment { id: id.clone() });
        }
        DisagreementSource::IndependenceViolation { circular } => {
            settlements.push(Settlement::IndependentReview {
                reason: format!(
                    "{} of the disputing oracles are not independent of the evaluated system",
                    circular.len()
                ),
            });
        }
        DisagreementSource::ScopeMismatch { .. } => {
            settlements.push(Settlement::IndependentReview {
                reason: "the disputing oracles establish disjoint planes and may not be answering \
                         the same question"
                    .to_string(),
            });
        }
        DisagreementSource::GenuineAmbiguity => {}
    }

    match tier.stronger() {
        Some(stronger) => settlements.push(Settlement::HigherTierOracle { at_least: stronger }),
        None => {
            settlements.push(Settlement::ArtifactRepair {
                pointer: String::new(),
            });
            settlements.push(Settlement::IndependentReview {
                reason: "two deterministic oracles contradict; at least one is defective"
                    .to_string(),
            });
        }
    }

    settlements
}

/// A challenge to a standing judgement (31.15, "support benchmark-result appeals").
///
/// An appeal is not a rerun. It records a *ground* for revisiting a result — the grader was buggy,
/// the specimen was swapped, the oracle has since been superseded — so that the eventual change of
/// score can be attributed to a cause, which is what 31.16's "report score movement by cause"
/// requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Appeal {
    pub against: OracleRef,
    pub filed_at: UtcTimestamp,
    pub grounds: AppealGrounds,
    pub outcome: Resolution,
}

impl Appeal {
    pub fn file(against: OracleRef, filed_at: UtcTimestamp, grounds: AppealGrounds) -> Self {
        Appeal {
            against,
            filed_at,
            grounds,
            outcome: Resolution::Open,
        }
    }

    pub fn resolve(mut self, outcome: Resolution) -> Self {
        self.outcome = outcome;
        self
    }
}

/// The admissible grounds for an appeal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "grounds", rename_all = "snake_case")]
pub enum AppealGrounds {
    /// The oracle itself computed the wrong thing.
    GraderDefect { detail: String },
    /// The evidence was not what it claimed to be — 31.15's specimen-swap case.
    ArtifactSwap { detail: String },
    /// A newer oracle version exists and would judge differently (31.16).
    VersionSuperseded { by: OracleRef },
    /// The oracle judged a plane it had declared it could not establish (40.21 invariant 1).
    ScopeExceeded { plane: String },
    /// Evidence that did not exist when the judgement was made.
    NewEvidence { detail: String },
}
