//! Aggregation that carries the disagreement (26.15, 26.20, 31.01).
//!
//! 26.15's design detail: "Expert disagreement is retained as an evaluation target. Consensus is
//! created only when the protocol explicitly calls for it and the process is recorded." 31.01
//! asks the same of oracles — "avoid consensus collapse when minority interpretations are
//! supported". Both are statements about what aggregation is allowed to destroy, and the answer
//! is nothing.
//!
//! Two aggregations live here and both refuse the mean.
//!
//! # A reader panel becomes a distribution, not a label
//!
//! Five readers, three calling progression and two calling treatment effect, do not produce
//! "progression". They produce `0.6 / 0.4`, and [`PanelAggregate::into_reference_standard`] hands
//! exactly that to [`crate::score`] as an uncertain reference — which is where the rest of the
//! crate's machinery picks it up. This is the loop that makes the design cohere: the panel's
//! disagreement does not get resolved before scoring, it gets *scored against*.
//!
//! # A lone dissenter on a safety-reaching class vetoes consensus
//!
//! 26.20 requires "task-specific vetoes" and 26.15's failure mode list includes "senior opinion
//! silently overrides distribution". One reader out of nine saying the laterality is wrong is not
//! 11% wrong; it is an unresolved claim that the output names the wrong side of a body, and no
//! majority makes it go away. [`ConsensusState::Vetoed`] is what the aggregate reports, and the
//! dissenter is named in it.
//!
//! # Pooling scores refuses to impute
//!
//! 26.20's failure mode "missing metrics imputed optimistically" is the reason
//! [`PooledScore::collapse`] fails whole rather than skipping the cases that refused. A mean over
//! the subset that happened to collapse is a mean over the easy cases, and it always reads high.
//!
//! # Not implemented
//!
//! No inter-rater reliability statistic — 26.15 lists it as a metric and κ or α needs a chance-
//! agreement model this module does not have. [`PanelAggregate::entropy_bits`] is offered as the
//! honest, model-free stand-in and is not a substitute. Pareto frontier computation over the
//! outcome vector of 26.20 is also absent; this module pools one dimension at a time and
//! deliberately does not know how to trade correctness against burden.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::AggregationError;
use crate::reference::{Dispersion, ReferenceDistribution, ReferenceStandard};
use crate::score::{BioScore, CollapsePolicy};
use crate::wrongness::{BiologicalErrorClass, Severity};

/// One reader's or oracle's position on a case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub rater: String,
    /// The state this rater called. Naming the same state space as the reference distribution is
    /// what lets the panel become one.
    pub position: String,
    /// Error classes this rater says the evaluated output committed. Empty for a rater who is
    /// only supplying a label.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flagged: Vec<BiologicalErrorClass>,
    /// Free-text rationale. 26.15 step 6 requires adjudication with recorded rationale; an
    /// unexplained dissent is harder to act on but is still retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

impl Rating {
    pub fn new(rater: impl Into<String>, position: impl Into<String>) -> Self {
        Rating {
            rater: rater.into(),
            position: position.into(),
            flagged: Vec::new(),
            rationale: None,
        }
    }

    pub fn flagging(mut self, class: BiologicalErrorClass) -> Self {
        self.flagged.push(class);
        self
    }

    pub fn because(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = Some(rationale.into());
        self
    }

    pub fn worst_flagged(&self) -> Option<Severity> {
        self.flagged.iter().map(|c| c.severity()).max()
    }
}

/// When a panel is permitted to declare consensus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsensusPolicy {
    pub policy_id: String,
    /// Share of raters that must hold one position before it counts as a majority.
    pub majority_threshold: f64,
    /// Whether a rater flagging a safety-reaching class blocks consensus outright.
    pub veto_on_safety_reaching: bool,
}

impl ConsensusPolicy {
    /// Two thirds, with safety vetoes active.
    pub fn conventional(policy_id: impl Into<String>) -> Self {
        ConsensusPolicy {
            policy_id: policy_id.into(),
            majority_threshold: 2.0 / 3.0,
            veto_on_safety_reaching: true,
        }
    }
}

/// A dissent that blocks consensus regardless of its size.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Veto {
    pub rater: String,
    pub class: BiologicalErrorClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

/// What the panel was able to conclude, if anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "consensus", rename_all = "snake_case")]
pub enum ConsensusState {
    Unanimous { position: String },
    /// A position cleared the threshold, and the dissenters are named. Not "the answer" — the
    /// answer is still the distribution.
    Majority {
        position: String,
        share: f64,
        dissenters: Vec<String>,
    },
    /// At least one rater flagged a safety-reaching class. Numerically the panel may be lopsided;
    /// it does not matter.
    Vetoed { by: Vec<Veto> },
    /// No position cleared the threshold.
    None { modal_share: f64 },
}

impl ConsensusState {
    /// The position a caller may act on, or `None` when the panel did not license one.
    ///
    /// [`ConsensusState::Majority`] does yield a position, but reading it discards the named
    /// dissenters — which is why the aggregate keeps them and this returns `Option`, not `String`.
    pub fn actionable_position(&self) -> Option<&str> {
        match self {
            ConsensusState::Unanimous { position } => Some(position),
            ConsensusState::Majority { position, .. } => Some(position),
            ConsensusState::Vetoed { .. } | ConsensusState::None { .. } => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ConsensusState::Unanimous { .. } => "unanimous",
            ConsensusState::Majority { .. } => "majority",
            ConsensusState::Vetoed { .. } => "vetoed",
            ConsensusState::None { .. } => "none",
        }
    }
}

/// A panel with every rating retained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelAggregate {
    policy_id: String,
    ratings: Vec<Rating>,
    tally: BTreeMap<String, Vec<String>>,
    consensus: ConsensusState,
}

impl PanelAggregate {
    /// Tallies a panel under a policy.
    ///
    /// Rejects duplicate raters: the same reader counted twice is the cheapest way to manufacture
    /// a majority, and 26.15's protocol calls for *independent* ratings.
    pub fn tally(
        policy: &ConsensusPolicy,
        ratings: impl IntoIterator<Item = Rating>,
    ) -> Result<Self, AggregationError> {
        let ratings: Vec<Rating> = ratings.into_iter().collect();
        if ratings.is_empty() {
            return Err(AggregationError::EmptyPanel);
        }

        let mut tally: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for rating in &ratings {
            if ratings.iter().filter(|r| r.rater == rating.rater).count() > 1 {
                return Err(AggregationError::DuplicateRater {
                    rater: rating.rater.clone(),
                });
            }
            tally
                .entry(rating.position.clone())
                .or_default()
                .push(rating.rater.clone());
        }

        let n = ratings.len() as f64;
        let vetoes: Vec<Veto> = if policy.veto_on_safety_reaching {
            ratings
                .iter()
                .flat_map(|r| {
                    r.flagged
                        .iter()
                        .filter(|c| c.is_safety_reaching())
                        .map(|&class| Veto {
                            rater: r.rater.clone(),
                            class,
                            rationale: r.rationale.clone(),
                        })
                })
                .collect()
        } else {
            Vec::new()
        };

        let (top_position, top_raters) = tally
            .iter()
            .max_by(|a, b| a.1.len().cmp(&b.1.len()).then_with(|| b.0.cmp(a.0)))
            .map(|(p, r)| (p.clone(), r.len()))
            .expect("a non-empty panel has at least one tallied position");
        let share = top_raters as f64 / n;

        let consensus = if !vetoes.is_empty() {
            ConsensusState::Vetoed { by: vetoes }
        } else if tally.len() == 1 {
            ConsensusState::Unanimous {
                position: top_position,
            }
        } else if share >= policy.majority_threshold {
            ConsensusState::Majority {
                dissenters: ratings
                    .iter()
                    .filter(|r| r.position != top_position)
                    .map(|r| r.rater.clone())
                    .collect(),
                position: top_position,
                share,
            }
        } else {
            ConsensusState::None { modal_share: share }
        };

        Ok(PanelAggregate {
            policy_id: policy.policy_id.clone(),
            ratings,
            tally,
            consensus,
        })
    }

    pub fn policy_id(&self) -> &str {
        &self.policy_id
    }

    /// Every rating, unmodified. Nothing is summarised away.
    pub fn ratings(&self) -> &[Rating] {
        &self.ratings
    }

    pub fn consensus(&self) -> &ConsensusState {
        &self.consensus
    }

    pub fn raters_for(&self, position: &str) -> &[String] {
        self.tally.get(position).map_or(&[], Vec::as_slice)
    }

    /// The empirical distribution over positions.
    pub fn distribution(&self) -> BTreeMap<String, f64> {
        let n = self.ratings.len() as f64;
        self.tally
            .iter()
            .map(|(position, raters)| (position.clone(), raters.len() as f64 / n))
            .collect()
    }

    /// Shannon entropy of the panel in bits: 0 for unanimity, rising with genuine split.
    ///
    /// Offered instead of an inter-rater reliability coefficient, which needs a chance-agreement
    /// model this module does not have. It is not κ and should not be reported as κ.
    pub fn entropy_bits(&self) -> f64 {
        let n = self.ratings.len() as f64;
        self.tally
            .values()
            .map(|raters| raters.len() as f64 / n)
            .filter(|&p| p > 0.0)
            .map(|p| -p * p.log2())
            .sum()
    }

    /// Positions held by fewer raters than the modal one, with who held them.
    ///
    /// 31.01: minority interpretations that are supported must survive. This is where they are
    /// read off.
    pub fn minority_positions(&self) -> Vec<(&str, &[String])> {
        let top = self.tally.values().map(Vec::len).max().unwrap_or(0);
        self.tally
            .iter()
            .filter(|(_, raters)| raters.len() < top)
            .map(|(position, raters)| (position.as_str(), raters.as_slice()))
            .collect()
    }

    pub fn vetoes(&self) -> &[Veto] {
        match &self.consensus {
            ConsensusState::Vetoed { by } => by,
            _ => &[],
        }
    }

    /// Hands the panel to the scoring machinery as an uncertain reference standard.
    ///
    /// `dispersion` must be supplied by the caller because only the panel's designers know
    /// whether the readers split because the biology is mixed or because the rubric was vague —
    /// 31.01's aleatoric-versus-annotation-error distinction. Passing
    /// [`Dispersion::Unattributed`] is allowed and honest; it simply means no scalar score can be
    /// collapsed from the result.
    pub fn into_reference_standard(
        self,
        dispersion: Dispersion,
    ) -> Result<ReferenceStandard, crate::error::ReferenceError> {
        let distribution = ReferenceDistribution::new(self.distribution(), dispersion)?;
        Ok(ReferenceStandard::Distribution(distribution))
    }
}

/// Several scores pooled without collapsing any of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PooledScore {
    requirement_id: String,
    scores: Vec<BioScore>,
}

impl PooledScore {
    /// Pools scores that passed the same comparability gate.
    ///
    /// Mixing gates is refused. 26.20 permits normalising "only within defensible groups", and a
    /// group defined by "these happened to be in the same result file" is not one.
    pub fn pool(scores: impl IntoIterator<Item = BioScore>) -> Result<Self, AggregationError> {
        let scores: Vec<BioScore> = scores.into_iter().collect();
        let Some(first) = scores.first() else {
            return Err(AggregationError::EmptyPanel);
        };
        let requirement_id = first.requirement_id().to_string();
        if let Some(odd) = scores
            .iter()
            .find(|s| s.requirement_id() != requirement_id)
        {
            return Err(AggregationError::MixedRequirements {
                expected: requirement_id,
                found: odd.requirement_id().to_string(),
            });
        }
        Ok(PooledScore {
            requirement_id,
            scores,
        })
    }

    pub fn requirement_id(&self) -> &str {
        &self.requirement_id
    }

    pub fn len(&self) -> usize {
        self.scores.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }

    pub fn scores(&self) -> &[BioScore] {
        &self.scores
    }

    /// How many pooled cases carry a critical error.
    ///
    /// Reported alongside any collapsed mean, never inside it. A pool whose mean is 0.9 and whose
    /// critical count is 3 is not a 0.9.
    pub fn critical_error_count(&self) -> usize {
        self.scores.iter().filter(|s| s.has_critical_error()).count()
    }

    pub fn clean_pass_count(&self) -> usize {
        self.scores.iter().filter(|s| s.is_clean_pass()).count()
    }

    /// The pooled band: the mean of the pessimistic ends and the mean of the optimistic ends.
    ///
    /// Still a band. Averaging the two ends first and then reporting one number would put the
    /// reference's uncertainty through exactly the collapse the crate exists to prevent.
    pub fn band(&self) -> (f64, f64) {
        let n = self.scores.len() as f64;
        let lo: f64 = self.scores.iter().map(|s| s.interval().lo()).sum::<f64>() / n;
        let hi: f64 = self.scores.iter().map(|s| s.interval().hi()).sum::<f64>() / n;
        (lo, hi)
    }

    /// The case whose reference was least able to decide. Worth looking at before any headline.
    pub fn widest_case(&self) -> Option<&BioScore> {
        self.scores.iter().max_by(|a, b| {
            a.interval()
                .width()
                .partial_cmp(&b.interval().width())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Collapses every pooled score under one policy and averages, or fails whole.
    ///
    /// The all-or-nothing behaviour is the point. Dropping the cases that refused to collapse
    /// would silently restrict the mean to the cases with confident references, which is 26.20's
    /// "missing metrics imputed optimistically" with extra steps.
    pub fn collapse(&self, policy: &CollapsePolicy) -> Result<f64, AggregationError> {
        let mut total = 0.0;
        for score in &self.scores {
            let value = score.collapse(policy).map_err(|e| {
                AggregationError::CaseNotCollapsible {
                    subject: score.subject().to_string(),
                    detail: e.to_string(),
                }
            })?;
            total += value;
        }
        Ok(total / self.scores.len() as f64)
    }
}
