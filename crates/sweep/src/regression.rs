//! Regression-focused scheduling: risk that rises with ignorance.
//!
//! Implements blueprint 08.06 (Regression-Focused Scheduling), the one module of §08 that
//! `bioprism-adaptive` does not own. That crate has the posterior, the information-gain selection,
//! the coverage floors, the stopping rule and the reproducible selection (08.01–08.05, 08.07,
//! 08.08). What it does not have is the case where the question is not "how good is this
//! architecture" but "what did this change break", and the two want opposite things: an adaptive
//! panel samples where it will learn most about capability, a regression panel samples where the
//! change could have done damage.
//!
//! # The term that makes the module worth writing
//!
//! 08.06's risk score is "change magnitude, component criticality, historical fragility, **unknown
//! coverage**, and potential side-effect severity".
//!
//! Unknown coverage. A component with a long history of never failing and a component nobody ever
//! tested both have zero recorded failures, and a fragility term computed from failure counts
//! ranks them identically — in fact it ranks the untested one *safer*, since it also has zero
//! recorded flakes. That is precisely backwards, and it is the default behaviour of every
//! regression heuristic that treats missing history as good history.
//!
//! [`Fragility`] therefore has two variants and [`Fragility::Unknown`] contributes the **maximum**
//! risk term rather than the minimum. [`rank`] then puts an untested component above a
//! well-tested reliable one, which is the module's central test.
//!
//! # No invented constants
//!
//! 08.06 names five risk factors and supplies not one number: no weights, no scale, and — for the
//! guardrail — "at least a fixed percentage of budget remains broad" with the percentage left
//! unstated.
//!
//! So [`Weights`] has no default worth the name. [`Weights::uniform`] exists and is documented as
//! **illustrative**; every other weighting is the caller's. The sentinel floor is a required
//! argument to [`Plan::validate`], and a plan validated without one is refused with
//! [`SweepError::UndeclaredPrecondition`] rather than checked against a number this crate made up.
//! The only numbers in this module are 0 and 1, which are the ends of the normalised scale, and
//! the arithmetic is a weighted sum stated in the docs so a reader can see it is a weighted sum and
//! not a model.
//!
//! # What is not implemented
//!
//! No diffing. [`ChangeFingerprint`] is told what changed; it does not compute it from two trees.
//! No learned weights: 08.06's feedback loop adjusts an integer selection weight up on a confirmed
//! regression and flags a mapping review on a false alarm, and that is the whole of the learning —
//! there is no model, and calling an increment "learning from past regressions" would overstate it.
//! No cost model, so [`Plan`] apportions a budget by count of cells rather than by expected spend.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};

/// The seven kinds of thing a change can touch. 08.06's change fingerprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Component,
    Prompt,
    Dependency,
    ToolSchema,
    Policy,
    ModelVersion,
    ProviderSetting,
}

impl ChangeKind {
    pub const ALL: [ChangeKind; 7] = [
        ChangeKind::Component,
        ChangeKind::Prompt,
        ChangeKind::Dependency,
        ChangeKind::ToolSchema,
        ChangeKind::Policy,
        ChangeKind::ModelVersion,
        ChangeKind::ProviderSetting,
    ];
}

/// What a release changed, by kind and identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeFingerprint {
    changed: BTreeMap<String, BTreeSet<String>>,
}

impl ChangeFingerprint {
    pub fn new() -> Self {
        ChangeFingerprint::default()
    }

    pub fn changing(mut self, kind: ChangeKind, id: impl Into<String>) -> Self {
        self.changed
            .entry(format!("{kind:?}"))
            .or_default()
            .insert(id.into());
        self
    }

    pub fn touches(&self, kind: ChangeKind, id: &str) -> bool {
        self.changed
            .get(&format!("{kind:?}"))
            .is_some_and(|s| s.contains(id))
    }

    /// How many distinct things changed, across all kinds.
    pub fn magnitude(&self) -> usize {
        self.changed.values().map(BTreeSet::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.magnitude() == 0
    }
}

/// 08.06's six test tiers, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Cells that exercise the changed component directly.
    DirectComponent,
    /// Cells that exercise it in combination with others.
    Interaction,
    /// Cells that caught a regression before.
    HistoricalRegression,
    /// Cells whose failure would be severe regardless of the change.
    HighSeveritySafety,
    /// Broad coverage, unrelated to the change.
    StableSentinel,
    /// Random sample, unrelated to the change.
    Exploratory,
}

impl Tier {
    pub const ALL: [Tier; 6] = [
        Tier::DirectComponent,
        Tier::Interaction,
        Tier::HistoricalRegression,
        Tier::HighSeveritySafety,
        Tier::StableSentinel,
        Tier::Exploratory,
    ];

    /// Whether this tier is 08.06's guardrail: "at least a fixed percentage of budget remains broad
    /// to detect unexpected cross-component effects".
    pub fn is_broad(self) -> bool {
        matches!(self, Tier::StableSentinel | Tier::Exploratory)
    }
}

/// What history says about a component's reliability.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Fragility {
    /// A measured rate in `[0, 1]`, from trials that actually ran.
    Measured { rate: f64 },
    /// No trials. Contributes the maximum risk term, not the minimum.
    Unknown,
}

impl Fragility {
    /// Build a measured fragility, refusing values outside the unit interval.
    pub fn measured(rate: f64) -> Result<Self, SweepError> {
        if !(0.0..=1.0).contains(&rate) || rate.is_nan() {
            return Err(SweepError::malformed(
                "Fragility::measured",
                format!("rate {rate} is outside [0, 1]"),
            ));
        }
        Ok(Fragility::Measured { rate })
    }

    /// The risk contribution. `Unknown` is 1.0 — the top of the scale.
    ///
    /// This is the inversion that matters. A component with `Measured { rate: 0.0 }` contributes
    /// nothing; one nobody has tested contributes everything. They are not neighbours.
    pub fn term(self) -> f64 {
        match self {
            Fragility::Measured { rate } => rate,
            Fragility::Unknown => 1.0,
        }
    }

    pub fn is_known(self) -> bool {
        matches!(self, Fragility::Measured { .. })
    }
}

/// One candidate cell and everything the risk score reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub cell_id: String,
    pub tier: Tier,
    /// The components this cell exercises.
    pub components: Vec<String>,
    /// How much a failure here would matter, in `[0, 1]`, supplied by the caller.
    pub criticality: f64,
    /// How severe a side effect this cell could produce, in `[0, 1]`, supplied by the caller.
    pub side_effect_severity: f64,
    pub fragility: Fragility,
    /// 08.06's feedback: raised by a confirmed regression.
    pub selection_weight: u32,
}

impl Candidate {
    pub fn new(cell_id: impl Into<String>, tier: Tier) -> Result<Self, SweepError> {
        let cell_id = cell_id.into();
        require_nonempty(&cell_id, "Candidate", "cell_id")?;
        Ok(Candidate {
            cell_id,
            tier,
            components: Vec::new(),
            criticality: 0.0,
            side_effect_severity: 0.0,
            fragility: Fragility::Unknown,
            selection_weight: 1,
        })
    }

    pub fn exercising(mut self, component: impl Into<String>) -> Self {
        self.components.push(component.into());
        self
    }

    pub fn with_criticality(mut self, value: f64) -> Result<Self, SweepError> {
        self.criticality = unit_interval(value, "criticality")?;
        Ok(self)
    }

    pub fn with_side_effect_severity(mut self, value: f64) -> Result<Self, SweepError> {
        self.side_effect_severity = unit_interval(value, "side_effect_severity")?;
        Ok(self)
    }

    pub fn with_fragility(mut self, fragility: Fragility) -> Self {
        self.fragility = fragility;
        self
    }
}

fn unit_interval(value: f64, field: &'static str) -> Result<f64, SweepError> {
    if !(0.0..=1.0).contains(&value) || value.is_nan() {
        return Err(SweepError::malformed(field, format!("{value} is outside [0, 1]")));
    }
    Ok(value)
}

/// The five weights of 08.06's risk score.
///
/// The blueprint names the factors and supplies no numbers, so there is no correct default. What is
/// here is a *stated* weighting the caller can replace, not a tuned one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Weights {
    pub change_magnitude: f64,
    pub criticality: f64,
    pub fragility: f64,
    pub unknown_coverage: f64,
    pub side_effect_severity: f64,
}

impl Weights {
    /// Equal weight on all five factors.
    ///
    /// **Illustrative.** 08.06 supplies no weighting and none has been fitted here; this exists so
    /// the score can be computed at all, and any conclusion that depends on the weighting is a
    /// conclusion about this function rather than about the blueprint.
    pub fn uniform() -> Self {
        Weights {
            change_magnitude: 1.0,
            criticality: 1.0,
            fragility: 1.0,
            unknown_coverage: 1.0,
            side_effect_severity: 1.0,
        }
    }
}

/// A risk score with its terms kept separate.
///
/// The components are public so a reviewer can see which factor drove the ranking. A single number
/// that cannot be decomposed is not reviewable, and 08.06's feedback loop ("false alarms trigger
/// evaluator and mapping review") is a review of exactly these terms.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RiskScore {
    pub change_magnitude: f64,
    pub criticality: f64,
    pub fragility: f64,
    pub unknown_coverage: f64,
    pub side_effect_severity: f64,
    pub total: f64,
}

/// Score one candidate against one change.
///
/// The arithmetic is a weighted sum of five terms each in `[0, 1]`. Change magnitude is the share
/// of this cell's components that the change touched; unknown coverage is 1 when the cell's
/// fragility is unmeasured and 0 otherwise, so an untested cell picks up *both* the maximum
/// fragility term and the unknown-coverage term. That double count is deliberate: it is the
/// difference between "this fails sometimes" and "we have no idea whether this fails".
pub fn score(candidate: &Candidate, change: &ChangeFingerprint, weights: &Weights) -> RiskScore {
    let touched = candidate
        .components
        .iter()
        .filter(|c| change.touches(ChangeKind::Component, c))
        .count();
    let change_magnitude = if candidate.components.is_empty() {
        0.0
    } else {
        touched as f64 / candidate.components.len() as f64
    };
    let unknown_coverage = if candidate.fragility.is_known() { 0.0 } else { 1.0 };
    let fragility = candidate.fragility.term();
    let total = weights.change_magnitude * change_magnitude
        + weights.criticality * candidate.criticality
        + weights.fragility * fragility
        + weights.unknown_coverage * unknown_coverage
        + weights.side_effect_severity * candidate.side_effect_severity;
    RiskScore {
        change_magnitude,
        criticality: candidate.criticality,
        fragility,
        unknown_coverage,
        side_effect_severity: candidate.side_effect_severity,
        total,
    }
}

/// Rank candidates by risk, highest first.
///
/// Ties break on cell id so the ordering is total and deterministic — 08.08's reproducible
/// selection applies here too, and a ranking that depends on hash-map order is not reproducible.
pub fn rank(
    candidates: &[Candidate],
    change: &ChangeFingerprint,
    weights: &Weights,
) -> Vec<(String, RiskScore)> {
    let mut scored: Vec<(String, RiskScore)> = candidates
        .iter()
        .map(|c| (c.cell_id.clone(), score(c, change, weights)))
        .collect();
    scored.sort_by(|a, b| {
        b.1.total
            .partial_cmp(&a.1.total)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    scored
}

/// A selected regression panel.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    selected: Vec<(String, Tier)>,
}

impl Plan {
    pub fn new() -> Self {
        Plan::default()
    }

    pub fn selecting(mut self, cell_id: impl Into<String>, tier: Tier) -> Self {
        self.selected.push((cell_id.into(), tier));
        self
    }

    pub fn len(&self) -> usize {
        self.selected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// The share of the plan spent on broad tiers.
    pub fn broad_share(&self) -> Option<f64> {
        if self.selected.is_empty() {
            return None;
        }
        let broad = self.selected.iter().filter(|(_, t)| t.is_broad()).count();
        Some(broad as f64 / self.selected.len() as f64)
    }

    /// Check the guardrail against a floor the caller must supply.
    ///
    /// `floor` is `None` when no floor was declared, and that is an error rather than a pass:
    /// 08.06 requires a floor and does not say what it is, so a plan with no declared floor has not
    /// satisfied the guardrail — it has skipped it.
    pub fn validate(&self, floor: Option<f64>) -> Result<(), SweepError> {
        let floor = floor.ok_or(SweepError::UndeclaredPrecondition {
            operation: "regression plan validation",
            declaration: "a broad-sentinel budget floor (08.06 requires one and names no value)",
        })?;
        let floor = unit_interval(floor, "floor")?;
        let share = self.broad_share().ok_or_else(|| {
            SweepError::malformed("Plan::validate", "an empty plan has no broad share")
        })?;
        if share + f64::EPSILON < floor {
            return Err(SweepError::malformed(
                "Plan::validate",
                format!("broad share {share:.3} is below the declared floor {floor:.3}"),
            ));
        }
        Ok(())
    }
}

/// What the feedback loop did with an outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum Feedback {
    /// A confirmed regression: this cell is worth more next time.
    WeightRaised { cell_id: String, to: u32 },
    /// A false alarm: 08.06 sends the evaluator and the capability mapping for review rather than
    /// quietly lowering the weight, because the cell may be right and the mapping wrong.
    ReviewRequested { cell_id: String, reason: &'static str },
}

/// Record a confirmed regression against a candidate.
pub fn confirm_regression(candidate: &mut Candidate) -> Feedback {
    candidate.selection_weight = candidate.selection_weight.saturating_add(1);
    Feedback::WeightRaised {
        cell_id: candidate.cell_id.clone(),
        to: candidate.selection_weight,
    }
}

/// Record a false alarm. The weight is untouched.
pub fn false_alarm(candidate: &Candidate) -> Feedback {
    Feedback::ReviewRequested {
        cell_id: candidate.cell_id.clone(),
        reason: "false alarm: review the evaluator and the capability mapping",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change() -> ChangeFingerprint {
        ChangeFingerprint::new()
            .changing(ChangeKind::Component, "planner")
            .changing(ChangeKind::Prompt, "system")
    }

    #[test]
    fn the_change_kinds_and_tiers_are_the_blueprints_seven_and_six() {
        assert_eq!(ChangeKind::ALL.len(), 7);
        assert_eq!(Tier::ALL.len(), 6);
        assert_eq!(Tier::ALL.iter().filter(|t| t.is_broad()).count(), 2);
    }

    #[test]
    fn a_fingerprint_counts_distinct_things_across_kinds() {
        assert_eq!(change().magnitude(), 2);
        assert!(change().touches(ChangeKind::Component, "planner"));
        assert!(!change().touches(ChangeKind::Prompt, "planner"));
        assert!(ChangeFingerprint::new().is_empty());
    }

    #[test]
    fn an_untested_component_scores_riskier_than_one_measured_as_never_failing() {
        let untested = Candidate::new("cell-untested", Tier::DirectComponent)
            .unwrap()
            .exercising("planner");
        let reliable = Candidate::new("cell-reliable", Tier::DirectComponent)
            .unwrap()
            .exercising("planner")
            .with_fragility(Fragility::measured(0.0).unwrap());
        let ranked = rank(&[reliable, untested], &change(), &Weights::uniform());
        assert_eq!(ranked[0].0, "cell-untested");
        assert!(ranked[0].1.total > ranked[1].1.total);
    }

    #[test]
    fn unknown_fragility_contributes_the_top_of_the_scale_not_the_bottom() {
        assert_eq!(Fragility::Unknown.term(), 1.0);
        assert_eq!(Fragility::measured(0.0).unwrap().term(), 0.0);
        assert!(!Fragility::Unknown.is_known());
    }

    #[test]
    fn an_untested_cell_picks_up_both_the_fragility_and_the_unknown_coverage_term() {
        let candidate = Candidate::new("c", Tier::DirectComponent).unwrap().exercising("planner");
        let s = score(&candidate, &change(), &Weights::uniform());
        assert_eq!(s.fragility, 1.0);
        assert_eq!(s.unknown_coverage, 1.0);
        let flaky = Candidate::new("c", Tier::DirectComponent)
            .unwrap()
            .exercising("planner")
            .with_fragility(Fragility::measured(1.0).unwrap());
        let f = score(&flaky, &change(), &Weights::uniform());
        assert_eq!(f.fragility, 1.0);
        assert_eq!(f.unknown_coverage, 0.0);
        assert!(s.total > f.total);
    }

    #[test]
    fn change_magnitude_is_the_share_of_a_cells_components_the_change_touched() {
        let half = Candidate::new("c", Tier::Interaction)
            .unwrap()
            .exercising("planner")
            .exercising("retriever")
            .with_fragility(Fragility::measured(0.0).unwrap());
        assert_eq!(score(&half, &change(), &Weights::uniform()).change_magnitude, 0.5);
    }

    #[test]
    fn a_cell_touching_nothing_the_change_touched_scores_zero_on_magnitude() {
        let untouched = Candidate::new("c", Tier::StableSentinel)
            .unwrap()
            .exercising("unrelated")
            .with_fragility(Fragility::measured(0.0).unwrap());
        assert_eq!(score(&untouched, &change(), &Weights::uniform()).total, 0.0);
    }

    #[test]
    fn the_risk_score_keeps_its_terms_separate_so_a_ranking_can_be_reviewed() {
        let candidate = Candidate::new("c", Tier::HighSeveritySafety)
            .unwrap()
            .exercising("planner")
            .with_criticality(0.5)
            .unwrap()
            .with_side_effect_severity(0.25)
            .unwrap()
            .with_fragility(Fragility::measured(0.1).unwrap());
        let s = score(&candidate, &change(), &Weights::uniform());
        assert_eq!(s.criticality, 0.5);
        assert_eq!(s.side_effect_severity, 0.25);
        assert!((s.total - (1.0 + 0.5 + 0.1 + 0.0 + 0.25)).abs() < 1e-12);
    }

    #[test]
    fn criticality_outside_the_unit_interval_is_refused() {
        let c = Candidate::new("c", Tier::DirectComponent).unwrap();
        assert!(c.clone().with_criticality(1.5).is_err());
        assert!(c.clone().with_side_effect_severity(-0.1).is_err());
        assert!(Fragility::measured(2.0).is_err());
        assert!(Fragility::measured(f64::NAN).is_err());
    }

    #[test]
    fn ranking_ties_break_deterministically_on_cell_id() {
        let a = Candidate::new("b-cell", Tier::Exploratory).unwrap();
        let b = Candidate::new("a-cell", Tier::Exploratory).unwrap();
        let ranked = rank(&[a, b], &change(), &Weights::uniform());
        assert_eq!(ranked[0].0, "a-cell");
        assert_eq!(ranked[1].0, "b-cell");
    }

    #[test]
    fn a_plan_with_no_declared_sentinel_floor_is_refused_rather_than_checked_against_an_invented_one()
    {
        let plan = Plan::new().selecting("c1", Tier::DirectComponent);
        assert!(matches!(
            plan.validate(None),
            Err(SweepError::UndeclaredPrecondition { .. })
        ));
    }

    #[test]
    fn a_plan_spent_entirely_on_targeted_cells_fails_the_guardrail() {
        let plan = Plan::new()
            .selecting("c1", Tier::DirectComponent)
            .selecting("c2", Tier::Interaction)
            .selecting("c3", Tier::HistoricalRegression);
        assert_eq!(plan.broad_share(), Some(0.0));
        assert!(plan.validate(Some(0.2)).is_err());
    }

    #[test]
    fn a_plan_meeting_the_declared_floor_passes() {
        let plan = Plan::new()
            .selecting("c1", Tier::DirectComponent)
            .selecting("c2", Tier::StableSentinel)
            .selecting("c3", Tier::Exploratory)
            .selecting("c4", Tier::Interaction);
        assert_eq!(plan.broad_share(), Some(0.5));
        assert!(plan.validate(Some(0.5)).is_ok());
        assert_eq!(plan.len(), 4);
        assert!(!plan.is_empty());
    }

    #[test]
    fn an_empty_plan_has_no_broad_share_and_cannot_be_validated() {
        assert_eq!(Plan::new().broad_share(), None);
        assert!(Plan::new().validate(Some(0.2)).is_err());
    }

    #[test]
    fn a_confirmed_regression_raises_the_weight_and_a_false_alarm_does_not_lower_it() {
        let mut candidate = Candidate::new("c", Tier::HistoricalRegression).unwrap();
        assert_eq!(
            confirm_regression(&mut candidate),
            Feedback::WeightRaised { cell_id: "c".into(), to: 2 }
        );
        let before = candidate.selection_weight;
        assert!(matches!(false_alarm(&candidate), Feedback::ReviewRequested { .. }));
        assert_eq!(candidate.selection_weight, before);
    }

    #[test]
    fn a_candidate_needs_a_cell_id() {
        assert!(Candidate::new("  ", Tier::Exploratory).is_err());
    }

    #[test]
    fn the_only_weighting_this_crate_supplies_expresses_no_preference_among_the_five_factors() {
        let w = Weights::uniform();
        let all = [
            w.change_magnitude,
            w.criticality,
            w.fragility,
            w.unknown_coverage,
            w.side_effect_severity,
        ];
        assert!(
            all.iter().all(|v| *v == all[0]),
            "08.06 supplies no weighting; a non-uniform default would present a fitted number as spec"
        );
    }
}
