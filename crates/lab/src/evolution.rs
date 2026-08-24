//! Benchmark-gated self-improvement and evolution cards.
//!
//! Blueprint 09.10. Its evolution card *"records before/after metrics, intervals, affected
//! capabilities, regressions, new tests, policy changes, dependencies, and rollback handle"*, and
//! its evolvable surface stops short of "permission core, audit, secrets, benchmark splits, and
//! release rules".
//!
//! Read together with 09.11, those two requirements make one claim, and it is the claim this whole
//! crate is built to hold:
//!
//! > **An improvement measured on a holdout the system has already touched is not an improvement,
//! > and must not be representable as one.**
//!
//! # How that is enforced, and why it is not a check
//!
//! An [`EvolutionCard`] whose measurement surface was contaminated is **constructible** — you have
//! to be able to record that it happened, and a system that cannot record its own contamination
//! will simply not mention it. What a contaminated card cannot do is produce an
//! [`ImprovementClaim`].
//!
//! The mechanism is the type system rather than a validation pass:
//!
//! - [`ImprovementClaim`]'s only constructor is [`EvolutionCard::claim_improvement`], and its
//!   fields are private.
//! - It implements `Serialize` but **not** `Deserialize`, so a claim cannot be written by hand and
//!   fed into a report. Neither can [`EvolutionCard`], for the same reason: it holds
//!   [`CleanMeasurement`]s, which have no deserializer either. A card round-trips through JSON in
//!   one direction only — out.
//! - The clean arm of [`MeasurementSurface`] can only be filled with two [`CleanMeasurement`]s,
//!   and those are minted exclusively by [`crate::holdout::Holdout::measure`] after it has checked
//!   the configuration's whole lineage against the exposure ledger.
//!
//! So there is no path from "we tuned on the holdout" to a published improvement that does not go
//! through writing a new constructor. That is the same move `bioprism-scale` makes with
//! `NominalCount`, whose lack of a `Serialize` impl means an instance count cannot be published
//! without its effective size beside it.
//!
//! # Every card states what would falsify it
//!
//! `would_have_to_be_true` is mandatory and non-empty. A self-improvement record that names no
//! condition under which the improvement is not real is a press release; 09.10 asks for "expected
//! trade-offs" and this is the honest form of that field. [`EvolutionError::NoDefeaterStated`] is
//! what a card gets for leaving it blank.
//!
//! # Not implemented, deliberately
//!
//! No proposal *generation* — nothing here writes a change. No test generation: 09.10's "generate
//! tests" step needs a generator, and `bioprism-mutation` owns that machinery. No reviewer identity
//! or approval workflow; the reviewer gate 09.10 names is a process, and a card records that it was
//! required rather than pretending to enforce it. No confidence intervals: 09.10 asks the card to
//! record intervals and a point delta between two point measurements is all this crate can justify,
//! so it reports the delta and says nothing about its precision.

use crate::error::EvolutionError;
use crate::holdout::{CleanMeasurement, HoldoutId, Partition};
use crate::pareto::Direction;
use crate::space::{ConfigurationId, ProtectedSurface};
use serde::{Deserialize, Serialize};

/// The branch 09.10 asks every change to arrive as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeProposal {
    pub id: String,
    pub rationale: String,
    /// Failure clusters this change targets. 09.10's "collect failures" step feeds this.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_failure_clusters: Vec<String>,
    /// What actually changed, typically from [`crate::space::CandidateArchitecture::diff`].
    pub changed_artifacts: Vec<String>,
    /// Regression cells generated for this change.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regression_cells: Vec<String>,
    /// Protected surfaces the proposal admits to touching. Any entry refuses the card.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub touches_protected: Vec<ProtectedSurface>,
}

impl ChangeProposal {
    pub fn new(id: impl Into<String>, rationale: impl Into<String>) -> Self {
        ChangeProposal {
            id: id.into(),
            rationale: rationale.into(),
            target_failure_clusters: Vec::new(),
            changed_artifacts: Vec::new(),
            regression_cells: Vec::new(),
            touches_protected: Vec::new(),
        }
    }

    pub fn changing<I, S>(mut self, artifacts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.changed_artifacts = artifacts.into_iter().map(Into::into).collect();
        self
    }

    pub fn targeting<I, S>(mut self, clusters: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.target_failure_clusters = clusters.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_regression_cells<I, S>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.regression_cells = cells.into_iter().map(Into::into).collect();
        self
    }

    pub fn touching(mut self, surface: ProtectedSurface) -> Self {
        self.touches_protected.push(surface);
        self
    }
}

/// Why a card's measurement was not clean, kept verbatim.
///
/// The refusal is the [`crate::error::HoldoutError`] the ledger produced, not a paraphrase of it,
/// so a reader can tell "we had already selected on this holdout" apart from "this holdout is
/// retired" apart from "an ancestor was exposed". Those have different remedies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContaminationRecord {
    pub holdout: HoldoutId,
    pub configuration: ConfigurationId,
    pub refusal: crate::error::HoldoutError,
}

/// What a card was measured on.
///
/// Deliberately **not** `Deserialize`: the clean arm holds [`CleanMeasurement`]s, and a
/// deserializable surface would be a way to mint clean measurements from JSON.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "surface")]
pub enum MeasurementSurface {
    Clean {
        before: CleanMeasurement,
        after: CleanMeasurement,
    },
    Contaminated(ContaminationRecord),
}

impl MeasurementSurface {
    pub fn is_clean(&self) -> bool {
        matches!(self, MeasurementSurface::Clean { .. })
    }
}

/// The self-improvement record of 09.10.
///
/// `Serialize`-only, for the reason given in the module docs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvolutionCard {
    pub id: String,
    pub proposal: ChangeProposal,
    pub baseline: ConfigurationId,
    pub candidate: ConfigurationId,
    surface: MeasurementSurface,
    /// The bundle a rollback restores. 09.11's rollback restores a whole bundle, so this is a
    /// configuration id and never a patch.
    pub rollback_handle: ConfigurationId,
    /// What would have to be true for the improvement to be real. Never empty.
    pub would_have_to_be_true: Vec<String>,
}

impl EvolutionCard {
    /// Builds a card from two clean measurements.
    ///
    /// Checks that the measurements are of the configurations the card names, that the proposal
    /// touches no protected surface, that it names at least one changed artifact, and that at
    /// least one defeater is stated. All five are refusals rather than warnings: a card that fails
    /// any of them cannot be argued with later, because it does not say enough to be wrong.
    pub fn measured(
        id: impl Into<String>,
        proposal: ChangeProposal,
        before: CleanMeasurement,
        after: CleanMeasurement,
        rollback_handle: &ConfigurationId,
        would_have_to_be_true: Vec<String>,
    ) -> Result<Self, EvolutionError> {
        let id = id.into();
        if let Some(surface) = proposal.touches_protected.first() {
            return Err(EvolutionError::ProtectedSurface {
                card: id,
                surface: surface.as_str().to_string(),
            });
        }
        if proposal.changed_artifacts.is_empty() {
            return Err(EvolutionError::NoChangedArtifacts { card: id });
        }
        if would_have_to_be_true
            .iter()
            .all(|statement| statement.trim().is_empty())
        {
            return Err(EvolutionError::NoDefeaterStated(id));
        }
        before.delta_to(&after)?;
        let baseline = before.configuration().clone();
        let candidate = after.configuration().clone();
        Ok(EvolutionCard {
            id,
            proposal,
            baseline,
            candidate,
            surface: MeasurementSurface::Clean { before, after },
            rollback_handle: rollback_handle.clone(),
            would_have_to_be_true,
        })
    }

    /// Builds a card recording that the measurement surface was already burned.
    ///
    /// This constructor exists because the alternative is worse. A system that cannot record a
    /// contaminated attempt records nothing, and a change that was tried against a burned holdout
    /// then disappears from the history — leaving the next reader to conclude it was never tried.
    /// The card is real, auditable, archivable, and cannot yield an [`ImprovementClaim`].
    pub fn contaminated(
        id: impl Into<String>,
        proposal: ChangeProposal,
        baseline: &ConfigurationId,
        candidate: &ConfigurationId,
        record: ContaminationRecord,
        rollback_handle: &ConfigurationId,
        would_have_to_be_true: Vec<String>,
    ) -> Self {
        EvolutionCard {
            id: id.into(),
            proposal,
            baseline: baseline.clone(),
            candidate: candidate.clone(),
            surface: MeasurementSurface::Contaminated(record),
            rollback_handle: rollback_handle.clone(),
            would_have_to_be_true,
        }
    }

    pub fn surface(&self) -> &MeasurementSurface {
        &self.surface
    }

    pub fn is_clean(&self) -> bool {
        self.surface.is_clean()
    }

    /// The signed change on the card's metric, whether or not it is an improvement.
    ///
    /// `None` for a contaminated card: there is a number, but it is not a delta between two
    /// measurements, and returning it would invite it to be quoted.
    pub fn delta(&self) -> Option<f64> {
        match &self.surface {
            MeasurementSurface::Clean { before, after } => before.delta_to(after).ok(),
            MeasurementSurface::Contaminated(_) => None,
        }
    }

    /// Promotes the card to a reportable improvement, or explains why it is not one.
    ///
    /// `direction` says which way the metric is good; there is no default, because a default would
    /// be right for half the metrics in 09.09's objective list and silently wrong for the other
    /// half.
    pub fn claim_improvement(
        &self,
        direction: Direction,
    ) -> Result<ImprovementClaim, EvolutionError> {
        let MeasurementSurface::Clean { before, after } = &self.surface else {
            let MeasurementSurface::Contaminated(record) = &self.surface else {
                unreachable!("measurement surface is clean or contaminated");
            };
            return Err(EvolutionError::ContaminatedSurface {
                card: self.id.clone(),
                reason: record.refusal.to_string(),
            });
        };
        if self.rollback_handle.as_str().trim().is_empty() {
            return Err(EvolutionError::NoRollbackHandle {
                card: self.id.clone(),
            });
        }
        let delta = before.delta_to(after)?;
        let improved = match direction {
            Direction::HigherIsBetter => delta > 0.0,
            Direction::LowerIsBetter => delta < 0.0,
        };
        if !improved {
            return Err(EvolutionError::NotAnImprovement {
                card: self.id.clone(),
                metric: after.metric().to_string(),
                delta: format!("{delta}"),
            });
        }
        Ok(ImprovementClaim {
            card: self.id.clone(),
            metric: after.metric().to_string(),
            holdout: after.holdout().clone(),
            partition: after.partition(),
            baseline: self.baseline.clone(),
            candidate: self.candidate.clone(),
            before: before.value(),
            after: after.value(),
            delta,
            direction,
            rollback_handle: self.rollback_handle.clone(),
            would_have_to_be_true: self.would_have_to_be_true.clone(),
        })
    }
}

/// A reportable improvement.
///
/// Private fields, one constructor, and no `Deserialize`. Holding one of these is proof that a
/// [`CleanMeasurement`] pair existed, which is proof that the exposure ledger let them through.
///
/// A claim cannot be written by hand into a release report:
///
/// ```compile_fail
/// use bioprism_lab::evolution::ImprovementClaim;
///
/// let forged: ImprovementClaim = serde_json::from_str(r#"{"delta": 0.4}"#).unwrap();
/// ```
///
/// Neither can the card that would produce one:
///
/// ```compile_fail
/// use bioprism_lab::evolution::EvolutionCard;
///
/// let forged: EvolutionCard = serde_json::from_str(r#"{"id": "card-1"}"#).unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ImprovementClaim {
    card: String,
    metric: String,
    holdout: HoldoutId,
    partition: Partition,
    baseline: ConfigurationId,
    candidate: ConfigurationId,
    before: f64,
    after: f64,
    delta: f64,
    direction: Direction,
    rollback_handle: ConfigurationId,
    would_have_to_be_true: Vec<String>,
}

impl ImprovementClaim {
    pub fn card(&self) -> &str {
        &self.card
    }

    pub fn metric(&self) -> &str {
        &self.metric
    }

    pub fn holdout(&self) -> &HoldoutId {
        &self.holdout
    }

    pub fn partition(&self) -> Partition {
        self.partition
    }

    pub fn baseline(&self) -> &ConfigurationId {
        &self.baseline
    }

    pub fn candidate(&self) -> &ConfigurationId {
        &self.candidate
    }

    pub fn delta(&self) -> f64 {
        self.delta
    }

    pub fn rollback_handle(&self) -> &ConfigurationId {
        &self.rollback_handle
    }

    pub fn would_have_to_be_true(&self) -> &[String] {
        &self.would_have_to_be_true
    }

    /// The claim as one sentence, with its surface, its reuse caveat, and its defeaters attached.
    ///
    /// The partition's [`Partition::reuse_note`] is not optional decoration. A delta on the public
    /// evaluation set and a delta on the rotating private set are different claims, and a sentence
    /// that omits which one it was is the sentence that gets quoted.
    pub fn to_sentence(&self) -> String {
        format!(
            "`{}` moved `{}` from {} to {} ({:+}) on holdout `{}` ({}; {}). Rollback: `{}`. Not real if: {}.",
            self.candidate,
            self.metric,
            self.before,
            self.after,
            self.delta,
            self.holdout,
            self.partition.as_str(),
            self.partition.reuse_note(),
            self.rollback_handle,
            self.would_have_to_be_true.join("; ")
        )
    }
}

/// 09.10's archive: alternative variants are retained rather than overwritten.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct EvolutionArchive {
    cards: Vec<EvolutionCard>,
}

impl EvolutionArchive {
    pub fn new() -> Self {
        EvolutionArchive::default()
    }

    pub fn push(&mut self, card: EvolutionCard) {
        self.cards.push(card);
    }

    pub fn cards(&self) -> &[EvolutionCard] {
        &self.cards
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// Cards whose measurement surface was already burned. Counting them is the point: a run of
    /// contaminated cards means the holdout policy is being routed around, and that is a finding.
    pub fn contaminated(&self) -> Vec<&EvolutionCard> {
        self.cards.iter().filter(|card| !card.is_clean()).collect()
    }

    /// Every card that can be reported as an improvement in `direction`.
    pub fn improvements(&self, direction: Direction) -> Vec<ImprovementClaim> {
        self.cards
            .iter()
            .filter_map(|card| card.claim_improvement(direction).ok())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holdout::{Holdout, HoldoutLedger, Partition};
    use crate::space::{ArchitectureSpace, CandidateArchitecture, ComponentKind, ComponentSpec};

    fn minimal(id: &str) -> CandidateArchitecture {
        CandidateArchitecture::new(id)
            .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
            .with_component(ComponentSpec::new("run", ComponentKind::Executor))
            .with_component(ComponentSpec::new("stop", ComponentKind::Terminator))
    }

    fn proposal() -> ChangeProposal {
        ChangeProposal::new("p1", "widen the protected closure before relevance")
            .changing(["component `select` parameter `depth` 3 -> 5"])
            .targeting(["cluster:missing-closure"])
    }

    fn defeaters() -> Vec<String> {
        vec![
            "the gain survives on a second rotating private set".to_string(),
            "no capability regressed by more than the panel's own variation".to_string(),
        ]
    }

    fn clean_pair() -> (CleanMeasurement, CleanMeasurement) {
        let mut holdout = Holdout::new("private-a", Partition::RotatingPrivateCertification, 8);
        let before = holdout
            .measure(&[ConfigurationId::new("v1")], "admissible_rate", 0.70)
            .unwrap();
        let after = holdout
            .measure(&[ConfigurationId::new("v2")], "admissible_rate", 0.83)
            .unwrap();
        (before, after)
    }

    #[test]
    fn a_card_measured_on_a_clean_surface_reports_the_improvement() {
        let (before, after) = clean_pair();
        let card = EvolutionCard::measured(
            "card-1",
            proposal(),
            before,
            after,
            &ConfigurationId::new("v1"),
            defeaters(),
        )
        .unwrap();
        let claim = card.claim_improvement(Direction::HigherIsBetter).unwrap();
        assert!((claim.delta() - 0.13).abs() < 1e-9);
        assert!(claim
            .to_sentence()
            .contains("rotating_private_certification"));
    }

    #[test]
    fn a_card_whose_measurement_surface_was_contaminated_is_constructible() {
        let card = contaminated_card();
        assert!(!card.is_clean());
        assert_eq!(card.candidate, ConfigurationId::new("v2"));
    }

    #[test]
    fn a_contaminated_card_cannot_be_reported_as_an_improvement() {
        let card = contaminated_card();
        let error = card
            .claim_improvement(Direction::HigherIsBetter)
            .unwrap_err();
        assert!(matches!(
            error,
            EvolutionError::ContaminatedSurface { ref card, .. } if card == "card-2"
        ));
    }

    #[test]
    fn a_contaminated_card_reports_no_delta_at_all() {
        assert_eq!(contaminated_card().delta(), None);
    }

    fn contaminated_card() -> EvolutionCard {
        let mut space = ArchitectureSpace::new();
        space.register(minimal("v1")).unwrap();
        space.register(minimal("v2").derived_from("v1")).unwrap();
        let mut ledger = HoldoutLedger::new();
        ledger
            .register(Holdout::new(
                "private-a",
                Partition::RotatingPrivateCertification,
                8,
            ))
            .unwrap();
        let holdout = HoldoutId::new("private-a");
        ledger
            .record_selection(&holdout, &ConfigurationId::new("v1"), "tuned here")
            .unwrap();
        let refusal = ledger
            .measure(
                &holdout,
                &space,
                &ConfigurationId::new("v2"),
                "admissible_rate",
                0.83,
            )
            .unwrap_err();
        EvolutionCard::contaminated(
            "card-2",
            proposal(),
            &ConfigurationId::new("v1"),
            &ConfigurationId::new("v2"),
            ContaminationRecord {
                holdout,
                configuration: ConfigurationId::new("v2"),
                refusal,
            },
            &ConfigurationId::new("v1"),
            defeaters(),
        )
    }

    #[test]
    fn a_card_that_states_no_defeater_is_refused() {
        let (before, after) = clean_pair();
        assert_eq!(
            EvolutionCard::measured(
                "card-3",
                proposal(),
                before,
                after,
                &ConfigurationId::new("v1"),
                Vec::new(),
            ),
            Err(EvolutionError::NoDefeaterStated("card-3".to_string()))
        );
    }

    #[test]
    fn a_card_that_proposes_to_change_the_benchmark_splits_is_refused() {
        let (before, after) = clean_pair();
        let error = EvolutionCard::measured(
            "card-4",
            proposal().touching(ProtectedSurface::BenchmarkSplits),
            before,
            after,
            &ConfigurationId::new("v1"),
            defeaters(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            EvolutionError::ProtectedSurface { ref surface, .. } if surface == "benchmark_splits"
        ));
    }

    #[test]
    fn a_card_that_names_no_changed_artifact_is_refused() {
        let (before, after) = clean_pair();
        assert_eq!(
            EvolutionCard::measured(
                "card-5",
                ChangeProposal::new("p", "vibes"),
                before,
                after,
                &ConfigurationId::new("v1"),
                defeaters(),
            ),
            Err(EvolutionError::NoChangedArtifacts {
                card: "card-5".to_string()
            })
        );
    }

    #[test]
    fn a_regression_is_not_an_improvement_in_either_direction_by_accident() {
        let mut holdout = Holdout::new("private-a", Partition::RotatingPrivateCertification, 8);
        let before = holdout
            .measure(&[ConfigurationId::new("v1")], "admissible_rate", 0.90)
            .unwrap();
        let after = holdout
            .measure(&[ConfigurationId::new("v2")], "admissible_rate", 0.70)
            .unwrap();
        let card = EvolutionCard::measured(
            "card-6",
            proposal(),
            before,
            after,
            &ConfigurationId::new("v1"),
            defeaters(),
        )
        .unwrap();
        assert!(matches!(
            card.claim_improvement(Direction::HigherIsBetter),
            Err(EvolutionError::NotAnImprovement { .. })
        ));
        assert!(card.claim_improvement(Direction::LowerIsBetter).is_ok());
    }

    #[test]
    fn a_card_comparing_two_different_metrics_is_refused_before_it_exists() {
        let mut holdout = Holdout::new("private-a", Partition::RotatingPrivateCertification, 8);
        let before = holdout
            .measure(&[ConfigurationId::new("v1")], "admissible_rate", 0.70)
            .unwrap();
        let after = holdout
            .measure(&[ConfigurationId::new("v2")], "latency_units", 0.30)
            .unwrap();
        assert!(matches!(
            EvolutionCard::measured(
                "card-7",
                proposal(),
                before,
                after,
                &ConfigurationId::new("v1"),
                defeaters(),
            ),
            Err(EvolutionError::Holdout(_))
        ));
    }

    #[test]
    fn an_archive_counts_contaminated_attempts_rather_than_dropping_them() {
        let mut archive = EvolutionArchive::new();
        let (before, after) = clean_pair();
        archive.push(
            EvolutionCard::measured(
                "card-1",
                proposal(),
                before,
                after,
                &ConfigurationId::new("v1"),
                defeaters(),
            )
            .unwrap(),
        );
        archive.push(contaminated_card());
        assert_eq!(archive.len(), 2);
        assert_eq!(archive.contaminated().len(), 1);
        assert_eq!(archive.improvements(Direction::HigherIsBetter).len(), 1);
    }
}
