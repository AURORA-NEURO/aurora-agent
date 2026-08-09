//! Value of experiment — blueprint 34.10.
//!
//! 34.10 wants users to *"compare evidence-acquisition and next-experiment policies under tissue,
//! money, time, and privacy budgets"*, and lists *expected information gain* among its capabilities
//! and *VOI calibration* among its metrics. It defines neither.
//!
//! `bioprism-lab` hit the same wall at 09.02 and recorded the finding: "expected reduction in
//! harmful uncertainty ... divided by token, latency, monetary, and privacy cost" names no
//! estimator for the numerator and no exchange rate for the denominator. That finding is load
//! bearing here, because 34.10 is the *public* version — a ranked list on a hub page carries more
//! authority than a private plan, and inventing a number for it would be worse.
//!
//! So this module contributes the three things that are checkable without an estimator, and
//! refuses the fourth.
//!
//! # 1. A missing value is not a value of zero
//!
//! [`Experiment::value`] is an `Option`. There is no `value_or_zero`, no default and no imputation.
//! Both entry points return [`ValueError::ValueUndetermined`] naming the offending experiment
//! rather than quietly sorting it last — because sorting an unvalued experiment last is exactly the
//! claim that it is worthless, which is the opposite of what is known about it.
//!
//! # 2. Cost has no exchange rate, so the default comparison is a Pareto front
//!
//! Tissue, money and time are not commensurable, and 34.10 supplies no rate between them.
//! [`pareto_front`] therefore needs none: it reports which experiments are not dominated, which are,
//! and by whom. It is the honest answer to "which experiment next" when the answer is "these three,
//! and the choice between them is yours".
//!
//! [`rank_with`] produces the single ordered list a hub page actually wants, and can only be called
//! with an [`ExchangeRate`] that names who declared it and on what basis. The rate is an input the
//! caller owns; nothing in this crate supplies a default one, and there is no
//! `ExchangeRate::default`.
//!
//! # 3. Every output is labelled uncalibrated
//!
//! [`Calibration`] has exactly one variant. That is not an oversight: a `Calibrated` variant would
//! need evidence that declared values track realised information gain, which requires the realised
//! outcomes 34.10 lists as a capability and no mechanism to obtain. When such evidence exists, the
//! variant can be added along with the type that carries it.
//!
//! # Privacy is a boundary, not a denominator
//!
//! 34.10 lists privacy as a budget, which invites dividing by it. This module does not, for the
//! reason `bioprism-lab` gives: there is no exchange rate at which reading something you may not
//! read becomes worthwhile, and a cost model that offers one will eventually be handed a large
//! enough numerator. An experiment that crosses a boundary is [`Excluded`], and the exclusion is
//! reported rather than silently dropped so that a user can see what the policy cost them.
//!
//! # Not implemented
//!
//! No information-gain estimator, no belief state, no prior, no posterior, no realised-outcome
//! feedback and therefore no regret computation — 34.10 lists "policy regret" as a capability and
//! it requires an execution loop this crate does not have. No adaptive stopping rule: 09.02's
//! four-way stop lives in `bioprism-lab`, and duplicating it for a public surface would give two
//! answers to one question.

use crate::error::ValueError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Names a hypothesis the lab is trying to eliminate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HypothesisId(String);

impl HypothesisId {
    pub fn new(id: impl Into<String>) -> HypothesisId {
        HypothesisId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HypothesisId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Names a candidate experiment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExperimentId(String);

impl ExperimentId {
    pub fn new(id: impl Into<String>) -> ExperimentId {
        ExperimentId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExperimentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether a hypothesis is still worth spending on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HypothesisState {
    Open,
    /// Ruled out. An experiment that only addresses eliminated hypotheses buys nothing, however
    /// cheap it is.
    Eliminated,
}

/// One of 34.10's "hypothesis set".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub statement: String,
    pub state: HypothesisState,
}

impl Hypothesis {
    pub fn open(id: impl Into<String>, statement: impl Into<String>) -> Hypothesis {
        Hypothesis {
            id: HypothesisId::new(id),
            statement: statement.into(),
            state: HypothesisState::Open,
        }
    }

    pub fn eliminated(mut self) -> Hypothesis {
        self.state = HypothesisState::Eliminated;
        self
    }
}

/// Named cost axes and the amounts spent on them.
///
/// An open map rather than a struct with four fields, because 34.10 names tissue, money, time and
/// privacy while 34.03 names none, and a world may meter something neither lists. Privacy is
/// deliberately not among them: see [`Privacy`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Budget {
    axes: BTreeMap<String, u64>,
}

impl Budget {
    pub fn new() -> Budget {
        Budget::default()
    }

    pub fn spending(mut self, axis: impl Into<String>, amount: u64) -> Budget {
        self.axes.insert(axis.into(), amount);
        self
    }

    pub fn get(&self, axis: &str) -> u64 {
        self.axes.get(axis).copied().unwrap_or(0)
    }

    pub fn axes(&self) -> impl Iterator<Item = &str> {
        self.axes.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.axes.is_empty()
    }

    /// Whether every axis of this cost fits inside `available`, and if not, which axis broke first
    /// in a deterministic order.
    pub fn fits_within(&self, available: &Budget) -> Result<(), (String, u64, u64)> {
        for (axis, needed) in &self.axes {
            let have = available.get(axis);
            if *needed > have {
                return Err((axis.clone(), *needed, have));
            }
        }
        Ok(())
    }

    /// Whether this cost is no greater than `other` on every axis mentioned by either.
    fn no_worse_than(&self, other: &Budget) -> bool {
        self.axes
            .keys()
            .chain(other.axes.keys())
            .all(|axis| self.get(axis) <= other.get(axis))
    }
}

/// Whether an experiment stays inside the policy boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "privacy", rename_all = "snake_case")]
pub enum Privacy {
    WithinPolicy,
    /// The experiment would move or expose data it may not. Named rather than boolean so the
    /// exclusion can say which boundary.
    CrossesBoundary { boundary: String },
}

/// A number the caller wrote down, together with who wrote it and why.
///
/// Nothing in this crate produces one. That is the whole design: 34.10's "expected information
/// gain" has no estimator in the blueprint, and a value invented here would be indistinguishable
/// on a hub page from one that meant something.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclaredValue {
    value: f64,
    pub declared_by: String,
    pub rationale: String,
}

impl DeclaredValue {
    /// The only constructor, and it is named for what the number is not.
    pub fn uncalibrated(
        experiment: &ExperimentId,
        value: f64,
        declared_by: impl Into<String>,
        rationale: impl Into<String>,
    ) -> Result<DeclaredValue, ValueError> {
        if !value.is_finite() || value < 0.0 {
            return Err(ValueError::NonFiniteValue {
                experiment: experiment.to_string(),
            });
        }
        Ok(DeclaredValue {
            value,
            declared_by: declared_by.into(),
            rationale: rationale.into(),
        })
    }

    pub fn get(&self) -> f64 {
        self.value
    }
}

/// A candidate next experiment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Experiment {
    pub id: ExperimentId,
    /// Which hypotheses this experiment would bear on.
    pub addresses: BTreeSet<HypothesisId>,
    pub cost: Budget,
    pub privacy: Privacy,
    /// `None` means nobody could say what this is worth. It does not mean zero, and no code path
    /// in this module treats it as zero.
    pub value: Option<DeclaredValue>,
}

impl Experiment {
    pub fn new(id: impl Into<String>, cost: Budget) -> Experiment {
        Experiment {
            id: ExperimentId::new(id),
            addresses: BTreeSet::new(),
            cost,
            privacy: Privacy::WithinPolicy,
            value: None,
        }
    }

    pub fn addressing(mut self, hypothesis: &HypothesisId) -> Experiment {
        self.addresses.insert(hypothesis.clone());
        self
    }

    pub fn worth(mut self, value: DeclaredValue) -> Experiment {
        self.value = Some(value);
        self
    }

    pub fn crossing(mut self, boundary: impl Into<String>) -> Experiment {
        self.privacy = Privacy::CrossesBoundary {
            boundary: boundary.into(),
        };
        self
    }
}

/// Why a candidate was taken out of the comparison.
///
/// Every exclusion is reported rather than filtered silently, because a user who cannot see what
/// the policy removed will conclude the policy removed nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum ExclusionReason {
    /// Every hypothesis it addresses has already been eliminated.
    AddressesNothingOpen,
    /// Excluded, not discounted. There is no value that overcomes this.
    CrossesPrivacyBoundary { boundary: String },
    OverBudget {
        axis: String,
        needs: u64,
        available: u64,
    },
}

/// A candidate and the reason it is not in the comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Excluded {
    pub experiment: ExperimentId,
    pub reason: ExclusionReason,
}

/// The calibration status of every output of this module.
///
/// One variant. A `Calibrated` variant would require evidence that declared values track realised
/// information gain; 34.10 lists "realized oracle outcome" as a capability and gives no way to feed
/// it back. Adding the variant without that evidence would turn a label into a lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Calibration {
    Uncalibrated,
}

/// A caller-supplied price for each cost axis, and the caller's name against it.
///
/// No `Default`. No constructor that fills in weights. The point of the type is that somebody has
/// to sign for the rate, because the rate is the whole content of a scalar ranking and the
/// blueprint does not contain one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeRate {
    weights: BTreeMap<String, u64>,
    pub declared_by: String,
    pub basis: String,
}

impl ExchangeRate {
    pub fn declared(declared_by: impl Into<String>, basis: impl Into<String>) -> ExchangeRate {
        ExchangeRate {
            weights: BTreeMap::new(),
            declared_by: declared_by.into(),
            basis: basis.into(),
        }
    }

    pub fn pricing(mut self, axis: impl Into<String>, weight: u64) -> ExchangeRate {
        self.weights.insert(axis.into(), weight);
        self
    }

    fn weight_of(&self, axis: &str) -> Option<u64> {
        self.weights.get(axis).copied()
    }

    /// The scalar cost of a budget under this rate, or the axis that has no price.
    fn scalarise(&self, cost: &Budget) -> Result<u64, ValueError> {
        let mut total: u64 = 0;
        for axis in cost.axes() {
            match self.weight_of(axis) {
                None => {
                    return Err(ValueError::NoExchangeRate {
                        axes: cost.axes().count(),
                    })
                }
                Some(0) => {
                    return Err(ValueError::ZeroWeight {
                        axis: axis.to_string(),
                    })
                }
                Some(w) => total = total.saturating_add(w.saturating_mul(cost.get(axis))),
            }
        }
        Ok(total)
    }
}

/// One line of a scalar ranking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ranked {
    pub experiment: ExperimentId,
    pub declared_value: f64,
    pub scalar_cost: u64,
    /// `declared_value / scalar_cost`, or infinity when the experiment costs nothing on every
    /// priced axis. Infinity is kept rather than clamped: a free experiment with any value really
    /// does come first, and clamping would put it behind an arbitrary threshold.
    pub value_per_cost: f64,
}

/// A scalar ranking, plus everything it excluded and the rate that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ranking {
    pub ranked: Vec<Ranked>,
    pub excluded: Vec<Excluded>,
    pub rate: ExchangeRate,
    pub calibration: Calibration,
}

impl Ranking {
    /// The top candidate, or `None` when everything was excluded.
    pub fn best(&self) -> Option<&Ranked> {
        self.ranked.first()
    }
}

/// The non-dominated candidates, the dominated ones with their dominator, and the exclusions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParetoFront {
    pub front: Vec<ExperimentId>,
    /// Each dominated candidate paired with one candidate that dominates it. One witness is enough
    /// to explain the exclusion, and reporting all of them would bury the explanation.
    pub dominated: Vec<(ExperimentId, ExperimentId)>,
    pub excluded: Vec<Excluded>,
    pub calibration: Calibration,
}

/// Screens candidates and refuses if any survivor has no declared value.
fn admit<'a>(
    candidates: &'a [Experiment],
    hypotheses: &[Hypothesis],
    available: &Budget,
) -> Result<(Vec<&'a Experiment>, Vec<Excluded>), ValueError> {
    if candidates.is_empty() {
        return Err(ValueError::NoCandidates);
    }

    let known: BTreeMap<&HypothesisId, HypothesisState> =
        hypotheses.iter().map(|h| (&h.id, h.state)).collect();

    let mut ordered: Vec<&Experiment> = candidates.iter().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));

    let mut admitted = Vec::new();
    let mut excluded = Vec::new();

    for experiment in ordered {
        for hypothesis in &experiment.addresses {
            if !known.contains_key(hypothesis) {
                return Err(ValueError::UnknownHypothesis {
                    experiment: experiment.id.to_string(),
                    hypothesis: hypothesis.to_string(),
                });
            }
        }

        if let Privacy::CrossesBoundary { boundary } = &experiment.privacy {
            excluded.push(Excluded {
                experiment: experiment.id.clone(),
                reason: ExclusionReason::CrossesPrivacyBoundary {
                    boundary: boundary.clone(),
                },
            });
            continue;
        }

        let addresses_something_open = experiment
            .addresses
            .iter()
            .any(|h| known.get(h) == Some(&HypothesisState::Open));
        if !addresses_something_open {
            excluded.push(Excluded {
                experiment: experiment.id.clone(),
                reason: ExclusionReason::AddressesNothingOpen,
            });
            continue;
        }

        if let Err((axis, needs, have)) = experiment.cost.fits_within(available) {
            excluded.push(Excluded {
                experiment: experiment.id.clone(),
                reason: ExclusionReason::OverBudget {
                    axis,
                    needs,
                    available: have,
                },
            });
            continue;
        }

        if experiment.value.is_none() {
            return Err(ValueError::ValueUndetermined {
                experiment: experiment.id.to_string(),
            });
        }

        admitted.push(experiment);
    }

    Ok((admitted, excluded))
}

/// The comparison that needs no exchange rate.
///
/// A candidate `a` dominates `b` when `a` is worth at least as much, costs no more on every axis,
/// and is strictly better somewhere. Everything not dominated is on the front, and the caller
/// chooses among them with knowledge this module does not have.
pub fn pareto_front(
    candidates: &[Experiment],
    hypotheses: &[Hypothesis],
    available: &Budget,
) -> Result<ParetoFront, ValueError> {
    let (admitted, excluded) = admit(candidates, hypotheses, available)?;

    let dominates = |a: &Experiment, b: &Experiment| -> bool {
        let (Some(va), Some(vb)) = (&a.value, &b.value) else {
            return false;
        };
        let value_ok = va.get() >= vb.get();
        let cost_ok = a.cost.no_worse_than(&b.cost);
        let strictly_better = va.get() > vb.get() || !b.cost.no_worse_than(&a.cost);
        value_ok && cost_ok && strictly_better
    };

    let mut front = Vec::new();
    let mut dominated = Vec::new();
    for b in &admitted {
        match admitted.iter().find(|a| dominates(a, b)) {
            Some(a) => dominated.push((b.id.clone(), a.id.clone())),
            None => front.push(b.id.clone()),
        }
    }

    Ok(ParetoFront {
        front,
        dominated,
        excluded,
        calibration: Calibration::Uncalibrated,
    })
}

/// The single ordered list, available only to a caller who supplies an exchange rate.
///
/// Ties break by experiment id so the ordering is total and reproducible.
pub fn rank_with(
    candidates: &[Experiment],
    hypotheses: &[Hypothesis],
    available: &Budget,
    rate: &ExchangeRate,
) -> Result<Ranking, ValueError> {
    let (admitted, excluded) = admit(candidates, hypotheses, available)?;

    let mut ranked = Vec::new();
    for experiment in admitted {
        let scalar_cost = rate.scalarise(&experiment.cost)?;
        let declared_value = experiment
            .value
            .as_ref()
            .expect("admit refuses any survivor without a declared value")
            .get();
        let value_per_cost = if scalar_cost == 0 {
            f64::INFINITY
        } else {
            declared_value / scalar_cost as f64
        };
        ranked.push(Ranked {
            experiment: experiment.id.clone(),
            declared_value,
            scalar_cost,
            value_per_cost,
        });
    }

    ranked.sort_by(|a, b| {
        b.value_per_cost
            .total_cmp(&a.value_per_cost)
            .then_with(|| a.experiment.cmp(&b.experiment))
    });

    Ok(Ranking {
        ranked,
        excluded,
        rate: rate.clone(),
        calibration: Calibration::Uncalibrated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hypotheses() -> Vec<Hypothesis> {
        vec![
            Hypothesis::open("h1", "the marker is prognostic"),
            Hypothesis::open("h2", "the effect is site-specific"),
            Hypothesis::open("h3", "the assay drifts").eliminated(),
        ]
    }

    fn h(id: &str) -> HypothesisId {
        HypothesisId::new(id)
    }

    fn cost(tissue: u64, money: u64) -> Budget {
        Budget::new().spending("tissue", tissue).spending("money", money)
    }

    fn valued(id: &str, tissue: u64, money: u64, value: f64) -> Experiment {
        let experiment = Experiment::new(id, cost(tissue, money)).addressing(&h("h1"));
        let declared =
            DeclaredValue::uncalibrated(&experiment.id, value, "a.reviewer", "expert guess")
                .unwrap();
        experiment.worth(declared)
    }

    fn purse() -> Budget {
        cost(100, 100)
    }

    fn rate() -> ExchangeRate {
        ExchangeRate::declared("trial.board", "2026 protocol")
            .pricing("tissue", 3)
            .pricing("money", 1)
    }

    #[test]
    fn an_experiment_with_no_declared_value_is_refused_not_ranked_as_zero() {
        let candidates = vec![
            valued("a", 1, 1, 10.0),
            Experiment::new("b", cost(1, 1)).addressing(&h("h1")),
        ];
        let err = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap_err();
        assert_eq!(
            err,
            ValueError::ValueUndetermined {
                experiment: "b".to_string()
            }
        );
    }

    #[test]
    fn the_pareto_front_refuses_an_undetermined_value_too() {
        let candidates = vec![Experiment::new("b", cost(1, 1)).addressing(&h("h1"))];
        assert!(matches!(
            pareto_front(&candidates, &hypotheses(), &purse()),
            Err(ValueError::ValueUndetermined { .. })
        ));
    }

    #[test]
    fn the_refusal_names_the_first_offender_in_a_deterministic_order() {
        let candidates = vec![
            Experiment::new("z", cost(1, 1)).addressing(&h("h1")),
            Experiment::new("a", cost(1, 1)).addressing(&h("h1")),
        ];
        let err = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap_err();
        assert_eq!(
            err,
            ValueError::ValueUndetermined {
                experiment: "a".to_string()
            }
        );
    }

    #[test]
    fn an_experiment_that_crosses_a_privacy_boundary_is_excluded_not_discounted() {
        let candidates = vec![
            valued("a", 1, 1, 1.0),
            valued("b", 1, 1, 1_000_000.0).crossing("record-level export to a third site"),
        ];
        let ranking = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap();
        assert_eq!(ranking.ranked.len(), 1);
        assert_eq!(ranking.best().unwrap().experiment, ExperimentId::new("a"));
        assert!(matches!(
            ranking.excluded[0].reason,
            ExclusionReason::CrossesPrivacyBoundary { .. }
        ));
    }

    #[test]
    fn no_declared_value_however_large_buys_a_way_across_a_privacy_boundary() {
        let candidates = vec![valued("b", 0, 0, f64::MAX).crossing("pediatric subgroup")];
        let ranking = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap();
        assert!(ranking.ranked.is_empty());
        assert_eq!(ranking.excluded.len(), 1);
    }

    #[test]
    fn an_experiment_addressing_only_eliminated_hypotheses_is_excluded_with_a_reason() {
        let experiment = Experiment::new("a", cost(1, 1)).addressing(&h("h3"));
        let declared =
            DeclaredValue::uncalibrated(&experiment.id, 5.0, "r", "guess").unwrap();
        let candidates = vec![experiment.worth(declared)];
        let ranking = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap();
        assert_eq!(
            ranking.excluded[0].reason,
            ExclusionReason::AddressesNothingOpen
        );
    }

    #[test]
    fn an_experiment_over_budget_is_excluded_naming_the_axis_that_broke() {
        let candidates = vec![valued("a", 500, 1, 9.0)];
        let ranking = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap();
        assert_eq!(
            ranking.excluded[0].reason,
            ExclusionReason::OverBudget {
                axis: "tissue".to_string(),
                needs: 500,
                available: 100,
            }
        );
    }

    #[test]
    fn an_experiment_addressing_an_unknown_hypothesis_is_refused() {
        let candidates = vec![valued("a", 1, 1, 1.0).addressing(&h("h9"))];
        assert!(matches!(
            rank_with(&candidates, &hypotheses(), &purse(), &rate()),
            Err(ValueError::UnknownHypothesis { .. })
        ));
    }

    #[test]
    fn the_pareto_front_is_computable_without_any_exchange_rate() {
        let candidates = vec![
            valued("cheap-weak", 1, 1, 1.0),
            valued("dear-strong", 50, 50, 100.0),
            valued("dominated", 60, 60, 0.5),
        ];
        let front = pareto_front(&candidates, &hypotheses(), &purse()).unwrap();
        assert_eq!(
            front.front,
            vec![
                ExperimentId::new("cheap-weak"),
                ExperimentId::new("dear-strong")
            ]
        );
        assert_eq!(front.dominated.len(), 1);
        assert_eq!(front.dominated[0].0, ExperimentId::new("dominated"));
    }

    #[test]
    fn a_dominated_candidate_names_the_candidate_that_dominates_it() {
        let candidates = vec![valued("better", 1, 1, 10.0), valued("worse", 2, 2, 1.0)];
        let front = pareto_front(&candidates, &hypotheses(), &purse()).unwrap();
        assert_eq!(
            front.dominated,
            vec![(ExperimentId::new("worse"), ExperimentId::new("better"))]
        );
    }

    #[test]
    fn domination_is_irreflexive_so_a_lone_candidate_is_on_its_own_front() {
        let candidates = vec![valued("only", 3, 4, 7.0)];
        let front = pareto_front(&candidates, &hypotheses(), &purse()).unwrap();
        assert_eq!(front.front, vec![ExperimentId::new("only")]);
        assert!(front.dominated.is_empty());
    }

    #[test]
    fn identical_candidates_do_not_dominate_each_other_off_the_front() {
        let candidates = vec![valued("a", 2, 2, 5.0), valued("b", 2, 2, 5.0)];
        let front = pareto_front(&candidates, &hypotheses(), &purse()).unwrap();
        assert_eq!(front.front.len(), 2);
    }

    #[test]
    fn a_cost_axis_the_exchange_rate_does_not_price_is_refused() {
        let candidates = vec![valued("a", 1, 1, 1.0)];
        let partial = ExchangeRate::declared("board", "partial").pricing("tissue", 3);
        assert!(matches!(
            rank_with(&candidates, &hypotheses(), &purse(), &partial),
            Err(ValueError::NoExchangeRate { .. })
        ));
    }

    #[test]
    fn an_exchange_rate_that_prices_an_axis_at_zero_is_refused() {
        let candidates = vec![valued("a", 1, 1, 1.0)];
        let free_tissue = rate().pricing("tissue", 0);
        assert_eq!(
            rank_with(&candidates, &hypotheses(), &purse(), &free_tissue).unwrap_err(),
            ValueError::ZeroWeight {
                axis: "tissue".to_string()
            }
        );
    }

    #[test]
    fn every_output_is_labelled_uncalibrated_and_says_so_in_json() {
        let candidates = vec![valued("a", 1, 1, 1.0)];
        let ranking = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap();
        assert_eq!(ranking.calibration, Calibration::Uncalibrated);
        let json = serde_json::to_string(&ranking).unwrap();
        assert!(json.contains("\"uncalibrated\""));
        assert!(json.contains("trial.board"));
    }

    #[test]
    fn a_ranking_carries_the_exchange_rate_that_produced_it() {
        let candidates = vec![valued("a", 1, 1, 1.0)];
        let ranking = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap();
        assert_eq!(ranking.rate.declared_by, "trial.board");
        assert_eq!(ranking.rate.basis, "2026 protocol");
    }

    #[test]
    fn ranking_is_deterministic_under_ties() {
        let candidates = vec![
            valued("m", 1, 1, 4.0),
            valued("a", 1, 1, 4.0),
            valued("z", 1, 1, 4.0),
        ];
        let ids: Vec<_> = rank_with(&candidates, &hypotheses(), &purse(), &rate())
            .unwrap()
            .ranked
            .into_iter()
            .map(|r| r.experiment)
            .collect();
        assert_eq!(
            ids,
            vec![
                ExperimentId::new("a"),
                ExperimentId::new("m"),
                ExperimentId::new("z")
            ]
        );
    }

    #[test]
    fn a_free_experiment_with_any_value_ranks_first() {
        let candidates = vec![valued("free", 0, 0, 0.001), valued("dear", 1, 1, 999.0)];
        let ranking = rank_with(&candidates, &hypotheses(), &purse(), &rate()).unwrap();
        assert_eq!(ranking.best().unwrap().experiment, ExperimentId::new("free"));
        assert!(ranking.ranked[0].value_per_cost.is_infinite());
    }

    #[test]
    fn a_non_finite_declared_value_is_not_constructible() {
        let id = ExperimentId::new("a");
        assert!(DeclaredValue::uncalibrated(&id, f64::NAN, "r", "why").is_err());
        assert!(DeclaredValue::uncalibrated(&id, f64::INFINITY, "r", "why").is_err());
        assert!(DeclaredValue::uncalibrated(&id, -1.0, "r", "why").is_err());
        assert!(DeclaredValue::uncalibrated(&id, 0.0, "r", "why").is_ok());
    }

    #[test]
    fn an_empty_candidate_set_is_refused_rather_than_returning_an_empty_ranking() {
        assert_eq!(
            rank_with(&[], &hypotheses(), &purse(), &rate()).unwrap_err(),
            ValueError::NoCandidates
        );
    }
}
