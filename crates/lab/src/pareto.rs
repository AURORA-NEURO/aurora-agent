//! Pareto fronts, and the three answers dominance actually has.
//!
//! Blueprint 09.09: *"Retain non-dominated candidates plus diverse near-frontier variants. One
//! winner is not assumed across all task classes."* Its objective list is ten long — task success,
//! safety, calibration, cost, latency, reliability, complexity, provider portability, privacy,
//! interpretability — and the reason the module exists is that collapsing ten numbers into one
//! hides exactly the trade-off a reviewer needs to see.
//!
//! So this module has no scalarizer. There is no `score`, no weight vector, no `best()`, and no
//! tiebreak. [`compare`] returns one of four things, and the interesting one is
//! [`Dominance::Incomparable`]: **a candidate that is incomparable stays on the front**. That is
//! not indecision. Two architectures, one cheaper and one safer, are genuinely two answers, and a
//! function that returns one of them has thrown away the finding.
//!
//! # Unmeasured is not measured-and-poor
//!
//! `AGENTS.md` states this as non-negotiable and `bioprism-atlas` enforces it with a
//! `CapabilityCell` whose score is `Option<f64>` and whose unmeasured form has no score key at all.
//! An axis here follows the same rule: [`AxisValue::Unmeasured`] carries a
//! [`bioprism_atlas::UnmeasuredReason`] — the same vocabulary, imported rather than recreated —
//! and there is no `value_or_zero`.
//!
//! The consequence is stronger than it first looks. A candidate with an unmeasured axis can
//! neither dominate nor be dominated, whatever it does on the axes that *were* measured. It
//! therefore stays on the front, and [`ParetoFront::unresolved`] names it and the hole that put it
//! there. A front that quietly dropped it would be reporting "we compared these" about a
//! comparison nobody made; a front that scored the hole as zero would be reporting a measurement
//! nobody took.
//!
//! # Not implemented, deliberately
//!
//! No search. 09.09's loop — propose, statically validate, run a cheap cell panel, adaptively
//! evaluate, run the safety suite, hit the hidden holdout, confirm end to end — needs an executor
//! this crate does not have, and a *surrogate* model it explicitly says must never replace
//! certification. This module maintains the archive that such a loop would write into. No trial
//! allocation and no adaptive evaluation: `bioprism-adaptive` owns that. No confidence intervals
//! on an axis, so no statistical dominance — a point value is compared as a point value, and two
//! candidates differing by less than their unmeasured noise will read as a trade-off rather than a
//! tie, which is the conservative direction.

use crate::error::ParetoError;
use crate::space::ConfigurationId;
use bioprism_atlas::UnmeasuredReason;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Which way is better on an axis. Stated per objective because half of 09.09's list is a cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    HigherIsBetter,
    LowerIsBetter,
}

/// One axis of the multi-objective comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    pub axis: String,
    pub direction: Direction,
}

impl Objective {
    pub fn higher_is_better(axis: impl Into<String>) -> Self {
        Objective {
            axis: axis.into(),
            direction: Direction::HigherIsBetter,
        }
    }

    pub fn lower_is_better(axis: impl Into<String>) -> Self {
        Objective {
            axis: axis.into(),
            direction: Direction::LowerIsBetter,
        }
    }
}

/// A candidate's standing on one axis: a number, or a stated hole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum AxisValue {
    Measured { value: f64 },
    /// No measurement. Categorically distinct from a bad one, and there is no `value_or_zero`.
    Unmeasured { reason: UnmeasuredReason },
}

impl AxisValue {
    pub fn measured(value: f64) -> Self {
        AxisValue::Measured { value }
    }

    pub fn unmeasured(reason: UnmeasuredReason) -> Self {
        AxisValue::Unmeasured { reason }
    }

    /// The number, or `None`. The absence of a `value_or_zero` is the point of this type.
    pub fn value(&self) -> Option<f64> {
        match self {
            AxisValue::Measured { value } => Some(*value),
            AxisValue::Unmeasured { .. } => None,
        }
    }
}

/// One candidate's profile across every objective of a front.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub candidate: ConfigurationId,
    pub values: BTreeMap<String, AxisValue>,
}

impl Profile {
    pub fn new(candidate: &ConfigurationId) -> Self {
        Profile {
            candidate: candidate.clone(),
            values: BTreeMap::new(),
        }
    }

    pub fn measured(mut self, axis: impl Into<String>, value: f64) -> Self {
        self.values.insert(axis.into(), AxisValue::measured(value));
        self
    }

    pub fn unmeasured(mut self, axis: impl Into<String>, reason: UnmeasuredReason) -> Self {
        self.values
            .insert(axis.into(), AxisValue::unmeasured(reason));
        self
    }

    /// Axes with no measurement, with the reason each is a hole.
    pub fn holes(&self) -> Vec<(&str, UnmeasuredReason)> {
        self.values
            .iter()
            .filter_map(|(axis, value)| match value {
                AxisValue::Unmeasured { reason } => Some((axis.as_str(), *reason)),
                AxisValue::Measured { .. } => None,
            })
            .collect()
    }
}

/// Why two candidates could not be ordered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "incomparable_because")]
pub enum Incomparability {
    /// Each is better on at least one measured axis. A real finding, and the reason the archive
    /// keeps both.
    TradeOff {
        left_better_on: Vec<String>,
        right_better_on: Vec<String>,
    },
    /// At least one axis is unmeasured for one of them, so no ordering is knowable. Distinct from
    /// a trade-off because the fix is different: measure the axis.
    Unmeasured { axes: Vec<String> },
}

/// The four possible relations between two profiles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "relation")]
pub enum Dominance {
    /// Left is at least as good everywhere and strictly better somewhere.
    Dominates,
    DominatedBy,
    /// Identical on every measured axis.
    Equivalent,
    Incomparable(Incomparability),
}

impl Dominance {
    pub fn is_incomparable(&self) -> bool {
        matches!(self, Dominance::Incomparable(_))
    }
}

/// Compares two profiles over `objectives`.
///
/// Unmeasured wins over everything: if any axis is unmeasured for either candidate, the answer is
/// [`Incomparability::Unmeasured`] even when the measured axes would have settled it. That is
/// deliberate and it is the strict reading. "Better on the four axes we looked at" is not
/// domination, and the moment it is allowed to be, the cheapest way to dominate is to skip the
/// expensive axis.
pub fn compare(
    objectives: &[Objective],
    left: &Profile,
    right: &Profile,
) -> Result<Dominance, ParetoError> {
    if objectives.is_empty() {
        return Err(ParetoError::NoObjectives);
    }
    let mut unmeasured: Vec<String> = Vec::new();
    let mut left_better: Vec<String> = Vec::new();
    let mut right_better: Vec<String> = Vec::new();

    for objective in objectives {
        let l = value_of(left, &objective.axis)?;
        let r = value_of(right, &objective.axis)?;
        let (Some(l), Some(r)) = (l.value(), r.value()) else {
            unmeasured.push(objective.axis.clone());
            continue;
        };
        if !l.is_finite() {
            return Err(ParetoError::NonFiniteValue {
                candidate: left.candidate.to_string(),
                axis: objective.axis.clone(),
                value: format!("{l}"),
            });
        }
        if !r.is_finite() {
            return Err(ParetoError::NonFiniteValue {
                candidate: right.candidate.to_string(),
                axis: objective.axis.clone(),
                value: format!("{r}"),
            });
        }
        let left_is_better = match objective.direction {
            Direction::HigherIsBetter => l > r,
            Direction::LowerIsBetter => l < r,
        };
        let right_is_better = match objective.direction {
            Direction::HigherIsBetter => r > l,
            Direction::LowerIsBetter => r < l,
        };
        if left_is_better {
            left_better.push(objective.axis.clone());
        } else if right_is_better {
            right_better.push(objective.axis.clone());
        }
    }

    if !unmeasured.is_empty() {
        return Ok(Dominance::Incomparable(Incomparability::Unmeasured {
            axes: unmeasured,
        }));
    }
    Ok(match (left_better.is_empty(), right_better.is_empty()) {
        (true, true) => Dominance::Equivalent,
        (false, true) => Dominance::Dominates,
        (true, false) => Dominance::DominatedBy,
        (false, false) => Dominance::Incomparable(Incomparability::TradeOff {
            left_better_on: left_better,
            right_better_on: right_better,
        }),
    })
}

fn value_of<'a>(profile: &'a Profile, axis: &str) -> Result<&'a AxisValue, ParetoError> {
    profile
        .values
        .get(axis)
        .ok_or_else(|| ParetoError::AxisAbsent {
            candidate: profile.candidate.to_string(),
            axis: axis.to_string(),
        })
}

/// What happened when a candidate was offered to the archive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "admission")]
pub enum Admission {
    /// Kept. `displaced` names the members it dominated off the front.
    Admitted { displaced: Vec<ConfigurationId> },
    /// Refused, with the member that dominates it named. Never "refused" without a dominator: a
    /// candidate rejected by an unnamed rule is a candidate rejected by a bug.
    Dominated { by: ConfigurationId },
}

/// A candidate the archive keeps but cannot place, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unresolved {
    pub candidate: ConfigurationId,
    pub axes: Vec<String>,
    pub reasons: Vec<(String, UnmeasuredReason)>,
}

/// The result of asking the front for one deployable candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "selection")]
pub enum Selection {
    /// Exactly one member. The only case in which a caller may deploy without deciding anything.
    Unique { candidate: ConfigurationId },
    /// Several members, none dominating another. A real answer: the front is the finding, and
    /// picking among these is a judgement call this crate refuses to make for you.
    Ambiguous {
        front: Vec<ConfigurationId>,
        unresolved: Vec<Unresolved>,
    },
    Empty,
}

/// The non-dominated archive of 09.09.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParetoFront {
    objectives: Vec<Objective>,
    members: Vec<Profile>,
    /// Candidates removed or refused, each with the member that dominated it. Kept because
    /// "we tried that and it lost to this" is the cheapest fact to lose and the most annoying to
    /// rediscover.
    archived: Vec<(Profile, ConfigurationId)>,
}

impl ParetoFront {
    /// Builds an empty front, refusing an empty or duplicated objective set.
    pub fn new(objectives: Vec<Objective>) -> Result<Self, ParetoError> {
        if objectives.is_empty() {
            return Err(ParetoError::NoObjectives);
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for objective in &objectives {
            if !seen.insert(objective.axis.as_str()) {
                return Err(ParetoError::DuplicateObjective(objective.axis.clone()));
            }
        }
        Ok(ParetoFront {
            objectives,
            members: Vec::new(),
            archived: Vec::new(),
        })
    }

    pub fn objectives(&self) -> &[Objective] {
        &self.objectives
    }

    pub fn members(&self) -> &[Profile] {
        &self.members
    }

    pub fn archived(&self) -> &[(Profile, ConfigurationId)] {
        &self.archived
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Offers a candidate to the archive.
    ///
    /// Validates the profile against the objective set first: an axis the front does not know is
    /// an error, and an objective the profile says nothing about is an error too. The second is the
    /// one that matters — silence about an axis must be recorded as
    /// [`AxisValue::Unmeasured`] with a reason, because "we did not measure latency" and "latency
    /// was not part of this comparison" are different claims.
    pub fn insert(&mut self, profile: Profile) -> Result<Admission, ParetoError> {
        self.validate(&profile)?;
        if self
            .members
            .iter()
            .any(|member| member.candidate == profile.candidate)
        {
            return Err(ParetoError::DuplicateCandidate {
                candidate: profile.candidate.to_string(),
            });
        }

        for member in &self.members {
            if compare(&self.objectives, member, &profile)? == Dominance::Dominates {
                let dominator = member.candidate.clone();
                self.archived.push((profile, dominator.clone()));
                return Ok(Admission::Dominated { by: dominator });
            }
        }

        let mut displaced = Vec::new();
        let mut kept = Vec::new();
        for member in std::mem::take(&mut self.members) {
            if compare(&self.objectives, &profile, &member)? == Dominance::Dominates {
                displaced.push(member.candidate.clone());
                self.archived.push((member, profile.candidate.clone()));
            } else {
                kept.push(member);
            }
        }
        self.members = kept;
        self.members.push(profile);
        self.members
            .sort_by(|a, b| a.candidate.as_str().cmp(b.candidate.as_str()));
        Ok(Admission::Admitted { displaced })
    }

    fn validate(&self, profile: &Profile) -> Result<(), ParetoError> {
        let known: BTreeSet<&str> = self
            .objectives
            .iter()
            .map(|objective| objective.axis.as_str())
            .collect();
        for axis in profile.values.keys() {
            if !known.contains(axis.as_str()) {
                return Err(ParetoError::UnknownAxis {
                    candidate: profile.candidate.to_string(),
                    axis: axis.clone(),
                });
            }
        }
        for objective in &self.objectives {
            if !profile.values.contains_key(&objective.axis) {
                return Err(ParetoError::AxisAbsent {
                    candidate: profile.candidate.to_string(),
                    axis: objective.axis.clone(),
                });
            }
        }
        Ok(())
    }

    /// Members that are on the front partly because an axis was never measured.
    pub fn unresolved(&self) -> Vec<Unresolved> {
        self.members
            .iter()
            .filter_map(|member| {
                let holes = member.holes();
                if holes.is_empty() {
                    return None;
                }
                Some(Unresolved {
                    candidate: member.candidate.clone(),
                    axes: holes.iter().map(|(axis, _)| (*axis).to_string()).collect(),
                    reasons: holes
                        .iter()
                        .map(|(axis, reason)| ((*axis).to_string(), *reason))
                        .collect(),
                })
            })
            .collect()
    }

    /// Asks the front for a deployable candidate.
    ///
    /// Returns [`Selection::Ambiguous`] whenever more than one member survives, and there is no
    /// argument, weight or ordering that turns that into a [`Selection::Unique`]. A front with
    /// three members is the answer to the question that was asked.
    pub fn select(&self) -> Selection {
        match self.members.len() {
            0 => Selection::Empty,
            1 => Selection::Unique {
                candidate: self.members[0].candidate.clone(),
            },
            _ => Selection::Ambiguous {
                front: self
                    .members
                    .iter()
                    .map(|member| member.candidate.clone())
                    .collect(),
                unresolved: self.unresolved(),
            },
        }
    }

    /// The pairwise relation between two members, for a report that has to explain the front.
    pub fn relation(
        &self,
        left: &ConfigurationId,
        right: &ConfigurationId,
    ) -> Result<Dominance, ParetoError> {
        let find = |id: &ConfigurationId| {
            self.members
                .iter()
                .find(|member| &member.candidate == id)
                .ok_or_else(|| ParetoError::DuplicateCandidate {
                    candidate: id.to_string(),
                })
        };
        compare(&self.objectives, find(left)?, find(right)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objectives() -> Vec<Objective> {
        vec![
            Objective::higher_is_better("admissible_rate"),
            Objective::lower_is_better("cost_units"),
        ]
    }

    fn profile(id: &str, rate: f64, cost: f64) -> Profile {
        Profile::new(&ConfigurationId::new(id))
            .measured("admissible_rate", rate)
            .measured("cost_units", cost)
    }

    #[test]
    fn a_candidate_better_on_one_axis_and_worse_on_another_is_incomparable_not_ranked() {
        let cheap = profile("cheap", 0.80, 10.0);
        let good = profile("good", 0.95, 40.0);
        let relation = compare(&objectives(), &cheap, &good).unwrap();
        assert_eq!(
            relation,
            Dominance::Incomparable(Incomparability::TradeOff {
                left_better_on: vec!["cost_units".to_string()],
                right_better_on: vec!["admissible_rate".to_string()],
            })
        );
    }

    #[test]
    fn an_incomparable_candidate_stays_on_the_front() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        front.insert(profile("cheap", 0.80, 10.0)).unwrap();
        front.insert(profile("good", 0.95, 40.0)).unwrap();
        assert_eq!(front.len(), 2);
        assert!(matches!(front.select(), Selection::Ambiguous { .. }));
    }

    #[test]
    fn a_dominated_candidate_is_refused_with_its_dominator_named() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        front.insert(profile("good", 0.95, 10.0)).unwrap();
        assert_eq!(
            front.insert(profile("worse", 0.80, 40.0)).unwrap(),
            Admission::Dominated {
                by: ConfigurationId::new("good")
            }
        );
        assert_eq!(front.archived().len(), 1);
    }

    #[test]
    fn a_new_candidate_displaces_the_members_it_dominates_and_says_which() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        front.insert(profile("old", 0.70, 40.0)).unwrap();
        front.insert(profile("cheap", 0.60, 5.0)).unwrap();
        assert_eq!(
            front.insert(profile("new", 0.90, 20.0)).unwrap(),
            Admission::Admitted {
                displaced: vec![ConfigurationId::new("old")]
            }
        );
        assert_eq!(front.len(), 2);
    }

    #[test]
    fn an_unmeasured_axis_makes_a_candidate_incomparable_rather_than_worst() {
        let measured = profile("measured", 0.95, 10.0);
        let partial = Profile::new(&ConfigurationId::new("partial"))
            .measured("admissible_rate", 0.50)
            .unmeasured("cost_units", UnmeasuredReason::NotAttempted);
        assert_eq!(
            compare(&objectives(), &measured, &partial).unwrap(),
            Dominance::Incomparable(Incomparability::Unmeasured {
                axes: vec!["cost_units".to_string()]
            })
        );
    }

    #[test]
    fn a_candidate_with_an_unmeasured_axis_is_never_dominated_off_the_front() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        front
            .insert(
                Profile::new(&ConfigurationId::new("partial"))
                    .measured("admissible_rate", 0.10)
                    .unmeasured("cost_units", UnmeasuredReason::DeferredAcquisition),
            )
            .unwrap();
        front.insert(profile("strong", 0.99, 1.0)).unwrap();
        assert_eq!(front.len(), 2);
        let unresolved = front.unresolved();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].axes, vec!["cost_units".to_string()]);
    }

    #[test]
    fn skipping_the_expensive_axis_does_not_buy_domination() {
        let objectives = objectives();
        let skipped = Profile::new(&ConfigurationId::new("skipped"))
            .measured("admissible_rate", 1.0)
            .unmeasured("cost_units", UnmeasuredReason::NotAttempted);
        let honest = profile("honest", 0.99, 1.0);
        assert!(compare(&objectives, &skipped, &honest)
            .unwrap()
            .is_incomparable());
    }

    #[test]
    fn a_profile_that_says_nothing_about_an_objective_is_an_error_not_a_hole() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        let silent =
            Profile::new(&ConfigurationId::new("silent")).measured("admissible_rate", 0.9);
        assert_eq!(
            front.insert(silent),
            Err(ParetoError::AxisAbsent {
                candidate: "silent".to_string(),
                axis: "cost_units".to_string(),
            })
        );
    }

    #[test]
    fn a_front_with_two_members_has_no_selection_that_returns_one_of_them() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        front.insert(profile("cheap", 0.80, 10.0)).unwrap();
        front.insert(profile("good", 0.95, 40.0)).unwrap();
        let Selection::Ambiguous { front: members, .. } = front.select() else {
            panic!("a two-member front must not resolve to one candidate");
        };
        assert_eq!(members.len(), 2);
    }

    #[test]
    fn identical_profiles_are_equivalent_and_both_stay() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        front.insert(profile("a", 0.9, 10.0)).unwrap();
        front.insert(profile("b", 0.9, 10.0)).unwrap();
        assert_eq!(front.len(), 2);
        assert_eq!(
            front
                .relation(&ConfigurationId::new("a"), &ConfigurationId::new("b"))
                .unwrap(),
            Dominance::Equivalent
        );
    }

    #[test]
    fn a_front_with_no_objectives_is_refused_rather_than_making_everything_equivalent() {
        assert_eq!(ParetoFront::new(Vec::new()), Err(ParetoError::NoObjectives));
    }

    #[test]
    fn an_axis_the_front_does_not_declare_is_refused() {
        let mut front = ParetoFront::new(objectives()).unwrap();
        let extra = profile("extra", 0.9, 10.0).measured("interpretability", 0.5);
        assert_eq!(
            front.insert(extra),
            Err(ParetoError::UnknownAxis {
                candidate: "extra".to_string(),
                axis: "interpretability".to_string(),
            })
        );
    }
}
