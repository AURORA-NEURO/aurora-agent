//! Holdout partitions, exposure accounting, and the one type that can carry a score.
//!
//! Blueprint 09.11 asks for partitions with differing access rules and for query accounting:
//! *"Every architecture or prompt evaluated against a holdout increments an access ledger.
//! Repeated optimization against a set retires it as a true holdout."* 09.09 repeats the
//! requirement as its overfitting control, and 09.10 makes a hidden holdout a gate on every
//! self-improvement proposal.
//!
//! Those three sentences only mean something together, and what they mean is this:
//!
//! > **An improvement measured on a holdout the system has already touched is not an improvement.**
//!
//! A crate that enforces that with a boolean field called `contaminated` enforces nothing — the
//! field will be read in some places and not others, and the place it is not read is the release
//! report. So the enforcement here is the *absence of a value*:
//!
//! - [`CleanMeasurement`] has private fields and no public constructor. The only way to obtain one
//!   is [`Holdout::measure`], which checks exposure first.
//! - [`CleanMeasurement`] implements `Serialize` but **not** `Deserialize`. A score cannot be
//!   reconstituted from JSON, so it cannot enter a report without the ledger that vouched for it.
//!   This is the move `bioprism-scale` makes with `NominalCount`, applied to the other direction:
//!   there, a count cannot be published without its effective size; here, a score cannot be
//!   published without its exposure check.
//! - Contamination is a [`HoldoutError`], not a flag on a returned value. There is no
//!   `measure_anyway`, and no `Option<CleanMeasurement>` whose `None` arm a caller can ignore.
//!
//! # Exposure is monotone
//!
//! `exposure` is private and append-only. There is no `clear`, no `reset`, and no `set_exposure`.
//! [`crate::rollback`] restores a configuration bundle; it cannot restore an exposure ledger to an
//! earlier state, because a rollback that un-burns a holdout is the cheapest possible way to
//! launder a contaminated measurement.
//!
//! # Selection travels down lineage; measurement does not
//!
//! [`Holdout::measure`] takes the configuration's whole ancestor chain, not its id. If the holdout
//! was used to *select* the parent, the child inherits the burn — without this, laundering a
//! burned holdout costs one rename.
//!
//! Measuring a parent does not burn its children, and that asymmetry is deliberate rather than an
//! oversight: an evolution card needs a before and an after on one surface, so a rule under which
//! reading the baseline burned every descendant would make a clean card impossible. What that
//! leaves open is stated at [`ExposureKind::propagates_to_descendants`] and is the one hole in this
//! module that no ledger can close.
//!
//! # Not implemented, deliberately
//!
//! No clock — an [`ExposureEvent`] is ordered by an integer sequence number, not a timestamp, and
//! nothing here reads the system time. No significance testing: a [`CleanMeasurement`] is a point
//! value and this crate never claims an interval around it. No canary deployment, no shadow
//! routing, no traffic split and no automatic rollback trigger; 09.11 names all four and they need
//! a running system with live traffic, which this crate is not. No contextual bandit and no
//! exploration schedule — `bioprism-routing` already reports what its far simpler policy achieved,
//! and the answer was zero percent of available gain.

use crate::error::HoldoutError;
use crate::space::{ArchitectureSpace, ConfigurationId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Identifier of one evaluation partition instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HoldoutId(String);

impl HoldoutId {
    pub fn new(id: impl Into<String>) -> Self {
        HoldoutId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HoldoutId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The seven partitions of 09.11, whose "access and reuse rules differ".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Partition {
    /// Worlds and cells written to exercise the compiler. Optimized against freely.
    AuthoringFixtures,
    /// Data the context compiler was tuned on. Optimized against freely.
    CompilerTraining,
    /// The panel architecture search runs on. Optimized against freely, and therefore never a
    /// certification surface however good the number on it looks.
    ArchitectureDevelopment,
    /// Published tasks. Certifies, but every published query is permanent.
    PublicEvaluation,
    /// Withheld tasks rotated between releases. The strongest certification surface here.
    RotatingPrivateCertification,
    /// Adversarial tasks held for safety review.
    SafetyRedTeam,
    /// Failures observed in production. Not a certification surface: the sample is selected by
    /// what already went wrong, so a score on it measures the incident set, not the system.
    ProductionIncidents,
}

impl Partition {
    pub const ALL: [Partition; 7] = [
        Partition::AuthoringFixtures,
        Partition::CompilerTraining,
        Partition::ArchitectureDevelopment,
        Partition::PublicEvaluation,
        Partition::RotatingPrivateCertification,
        Partition::SafetyRedTeam,
        Partition::ProductionIncidents,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Partition::AuthoringFixtures => "authoring_fixtures",
            Partition::CompilerTraining => "compiler_training",
            Partition::ArchitectureDevelopment => "architecture_development",
            Partition::PublicEvaluation => "public_evaluation",
            Partition::RotatingPrivateCertification => "rotating_private_certification",
            Partition::SafetyRedTeam => "safety_red_team",
            Partition::ProductionIncidents => "production_incidents",
        }
    }

    /// Whether a score on this partition can ever be a certification measurement.
    ///
    /// A development partition returns `false` unconditionally. That is stronger than exposure
    /// accounting: even a first, untouched query against the architecture-development panel yields
    /// no [`CleanMeasurement`], because the panel exists to be optimized against and a number from
    /// it is a training score by construction.
    pub fn certifies(self) -> bool {
        matches!(
            self,
            Partition::PublicEvaluation
                | Partition::RotatingPrivateCertification
                | Partition::SafetyRedTeam
        )
    }

    /// The sentence a methods section must contain if it quotes a number from this partition.
    pub fn reuse_note(self) -> &'static str {
        match self {
            Partition::AuthoringFixtures => {
                "authoring fixtures were written to exercise the compiler and are optimized against"
            }
            Partition::CompilerTraining => "the compiler was tuned on this partition",
            Partition::ArchitectureDevelopment => {
                "architecture search ran against this panel; scores on it are training scores"
            }
            Partition::PublicEvaluation => {
                "published tasks; every query is permanent and the partition degrades with use"
            }
            Partition::RotatingPrivateCertification => {
                "withheld and rotated between releases; one query per configuration"
            }
            Partition::SafetyRedTeam => {
                "adversarial tasks held for safety review, not for capability comparison"
            }
            Partition::ProductionIncidents => {
                "sampled by what already failed; a score on it measures the incident set"
            }
        }
    }
}

/// Why a holdout was touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ExposureKind {
    /// A configuration was scored. Counts as a query and burns the holdout for it.
    Measurement { metric: String },
    /// A configuration was *chosen* on the strength of this holdout. The heaviest burn: the
    /// holdout is now part of the configuration.
    Selection { rationale: String },
    /// A cohort of candidates was compared against the holdout during search. Every member is
    /// burned, including the ones that lost — losing on a holdout is information about it too.
    Search { cohort_size: usize },
    /// A rollback restored an earlier configuration. Recorded so history shows that the burn
    /// survived the rollback, which is the point of recording it.
    Rollback { restored_to: ConfigurationId },
}

impl ExposureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExposureKind::Measurement { .. } => "measurement",
            ExposureKind::Selection { .. } => "selection",
            ExposureKind::Search { .. } => "search",
            ExposureKind::Rollback { .. } => "rollback",
        }
    }

    /// Whether the event consumes the holdout's query budget.
    ///
    /// A rollback does not: it spends no information about the holdout, and charging it would let
    /// an operator retire a holdout by restoring a bundle repeatedly.
    pub fn consumes_query_budget(&self) -> bool {
        !matches!(self, ExposureKind::Rollback { .. })
    }

    /// Whether the event burns the holdout for *descendants* of the configuration it names.
    ///
    /// `Selection` and `Search` do: those are the events in which the holdout's answer chose the
    /// configuration, so anything tuned from it carries the choice. `Measurement` does not, and
    /// that boundary is the one limitation in this module worth stating plainly.
    ///
    /// A measurement of a baseline has to be admissible, because 09.10's evolution card requires a
    /// before *and* an after on the same surface, and if reading the baseline burned every
    /// descendant then no card could ever be clean. The residual risk is real and is not closed by
    /// any ledger: somebody can read the baseline's score, see what it got wrong, and write the
    /// next configuration accordingly. That use of the holdout happens in a person's head and
    /// leaves no event to record. It is what an evolution card's `would_have_to_be_true` field
    /// exists to state, and it is why [`Partition::RotatingPrivateCertification`] is rotated rather
    /// than merely counted.
    pub fn propagates_to_descendants(&self) -> bool {
        matches!(
            self,
            ExposureKind::Selection { .. } | ExposureKind::Search { .. }
        )
    }
}

/// One append-only entry in the access ledger of 09.11.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureEvent {
    /// Position in the holdout's history. There is no timestamp here; see the module docs.
    pub seq: usize,
    pub configuration: ConfigurationId,
    pub kind: ExposureKind,
}

/// A position in a holdout's exposure history.
///
/// Carried by a [`crate::rollback::Checkpoint`] so a rollback can report how much exposure
/// accumulated since, which is the part of the world a rollback cannot undo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExposureWatermark(pub usize);

/// A score from a surface the configuration and its whole lineage had not touched.
///
/// # Why this type has no `Deserialize`
///
/// Cleanliness is a property of *how the number was obtained*, and JSON does not carry that. If
/// this type round-tripped, the shortest path to a clean measurement would be to write one by hand,
/// and every check in [`Holdout::measure`] would become optional. The type is `Serialize`-only so
/// a clean measurement can be published and audited but never re-entered.
///
/// A measurement serializes, so it can be published and audited:
///
/// ```
/// use bioprism_lab::holdout::{Holdout, Partition};
/// use bioprism_lab::space::ConfigurationId;
///
/// let mut holdout = Holdout::new("private-a", Partition::RotatingPrivateCertification, 4);
/// let measurement = holdout
///     .measure(&[ConfigurationId::new("v1")], "admissible_rate", 0.5)
///     .unwrap();
/// assert!(serde_json::to_string(&measurement).unwrap().contains("\"value\":0.5"));
/// ```
///
/// It does not deserialize, so it cannot be forged. The companion test above establishes that
/// `serde_json` itself is reachable from a doctest, so this one fails for the intended reason:
///
/// ```compile_fail
/// use bioprism_lab::holdout::CleanMeasurement;
///
/// let forged: CleanMeasurement = serde_json::from_str(r#"{"value": 1.0}"#).unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CleanMeasurement {
    holdout: HoldoutId,
    partition: Partition,
    configuration: ConfigurationId,
    metric: String,
    value: f64,
    /// The exposure event this measurement created. A reader can find it in the ledger.
    exposure_event: usize,
}

impl CleanMeasurement {
    pub fn holdout(&self) -> &HoldoutId {
        &self.holdout
    }

    pub fn partition(&self) -> Partition {
        self.partition
    }

    pub fn configuration(&self) -> &ConfigurationId {
        &self.configuration
    }

    pub fn metric(&self) -> &str {
        &self.metric
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn exposure_event(&self) -> usize {
        self.exposure_event
    }

    /// The signed change from `self` to `later`, refusing a comparison across metrics or surfaces.
    ///
    /// Returns a bare `f64` rather than a wrapper: a delta between two clean measurements on one
    /// surface is a number, and it is [`crate::evolution::ImprovementClaim`] that decides whether
    /// it may be called an improvement.
    pub fn delta_to(&self, later: &CleanMeasurement) -> Result<f64, HoldoutError> {
        if self.metric != later.metric {
            return Err(HoldoutError::MetricMismatch {
                left: self.metric.clone(),
                right: later.metric.clone(),
            });
        }
        if self.holdout != later.holdout {
            return Err(HoldoutError::SurfaceMismatch {
                left: self.holdout.to_string(),
                right: later.holdout.to_string(),
            });
        }
        Ok(later.value - self.value)
    }
}

/// One partition instance with its access ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Holdout {
    pub id: HoldoutId,
    pub partition: Partition,
    /// How many budget-consuming queries this holdout survives before 09.11 retires it.
    pub query_budget: u32,
    exposure: Vec<ExposureEvent>,
}

impl Holdout {
    pub fn new(id: impl Into<String>, partition: Partition, query_budget: u32) -> Self {
        Holdout {
            id: HoldoutId::new(id),
            partition,
            query_budget,
            exposure: Vec::new(),
        }
    }

    /// The access ledger. Read-only by construction: there is no mutable accessor.
    pub fn exposure(&self) -> &[ExposureEvent] {
        &self.exposure
    }

    pub fn watermark(&self) -> ExposureWatermark {
        ExposureWatermark(self.exposure.len())
    }

    pub fn queries_used(&self) -> u32 {
        self.exposure
            .iter()
            .filter(|event| event.kind.consumes_query_budget())
            .count() as u32
    }

    /// Whether repeated optimization has retired this set as a true holdout.
    pub fn is_retired(&self) -> bool {
        self.queries_used() >= self.query_budget
    }

    /// The first exposure event naming `configuration`, if any.
    pub fn exposure_for(&self, configuration: &ConfigurationId) -> Option<&ExposureEvent> {
        self.exposure
            .iter()
            .find(|event| &event.configuration == configuration)
    }

    pub fn is_burned_for(&self, configuration: &ConfigurationId) -> bool {
        self.selecting_exposure_for(configuration, false).is_some()
    }

    /// The first event naming `configuration` that burns the holdout for it.
    ///
    /// `inherited` narrows the question to events that reach descendants, per
    /// [`ExposureKind::propagates_to_descendants`].
    fn selecting_exposure_for(
        &self,
        configuration: &ConfigurationId,
        inherited: bool,
    ) -> Option<&ExposureEvent> {
        self.exposure.iter().find(|event| {
            &event.configuration == configuration
                && if inherited {
                    event.kind.propagates_to_descendants()
                } else {
                    event.kind.consumes_query_budget()
                }
        })
    }

    /// Records that a configuration was chosen using this holdout.
    ///
    /// Infallible on purpose. Selection is a thing that *happened*; refusing to record it would
    /// leave the ledger describing a world in which it did not. The refusal belongs at
    /// [`Holdout::measure`], where a claim is made.
    pub fn record_selection(&mut self, configuration: &ConfigurationId, rationale: &str) {
        self.append(
            configuration.clone(),
            ExposureKind::Selection {
                rationale: rationale.to_string(),
            },
        );
    }

    /// Records that a cohort of candidates was compared against this holdout.
    pub fn record_search(&mut self, cohort: &[ConfigurationId]) {
        let cohort_size = cohort.len();
        for configuration in cohort {
            self.append(configuration.clone(), ExposureKind::Search { cohort_size });
        }
    }

    /// Records that a rollback restored `restored_to`, without un-burning anything.
    pub fn record_rollback(&mut self, restored_to: &ConfigurationId) {
        self.append(
            restored_to.clone(),
            ExposureKind::Rollback {
                restored_to: restored_to.clone(),
            },
        );
    }

    /// Scores a configuration, or explains why the number would not be a measurement.
    ///
    /// `lineage` is the configuration and its ancestors, closest first — exactly what
    /// [`ArchitectureSpace::lineage`] returns. Passing only the configuration is legal and means
    /// "this configuration has no ancestors", which is a claim the caller is making.
    ///
    /// The checks run in the order a reader would want them reported: a development partition can
    /// never certify, whatever its exposure; a retired holdout is retired for everyone; and only
    /// then does the per-configuration burn matter.
    pub fn measure(
        &mut self,
        lineage: &[ConfigurationId],
        metric: &str,
        value: f64,
    ) -> Result<CleanMeasurement, HoldoutError> {
        let configuration = lineage
            .first()
            .ok_or_else(|| HoldoutError::UnknownHoldout(self.id.to_string()))?;

        if !self.partition.certifies() {
            return Err(HoldoutError::NotACertificationSurface {
                partition: self.partition.as_str().to_string(),
            });
        }
        if !value.is_finite() {
            return Err(HoldoutError::NonFiniteValue {
                holdout: self.id.to_string(),
                metric: metric.to_string(),
                value: format!("{value}"),
            });
        }
        if self.is_retired() {
            return Err(HoldoutError::Retired {
                holdout: self.id.to_string(),
                used: self.queries_used(),
                budget: self.query_budget,
            });
        }
        for (depth, ancestor) in lineage.iter().enumerate() {
            let Some(event) = self.selecting_exposure_for(ancestor, depth > 0) else {
                continue;
            };
            if depth > 0 {
                return Err(HoldoutError::AncestorExposed {
                    holdout: self.id.to_string(),
                    configuration: configuration.to_string(),
                    ancestor: ancestor.to_string(),
                });
            }
            return Err(match event.kind {
                ExposureKind::Selection { .. } | ExposureKind::Search { .. } => {
                    HoldoutError::SelectedUsingThisHoldout {
                        holdout: self.id.to_string(),
                        configuration: configuration.to_string(),
                        event: event.seq,
                    }
                }
                _ => HoldoutError::AlreadyQueried {
                    holdout: self.id.to_string(),
                    configuration: configuration.to_string(),
                },
            });
        }

        let seq = self.append(
            configuration.clone(),
            ExposureKind::Measurement {
                metric: metric.to_string(),
            },
        );
        Ok(CleanMeasurement {
            holdout: self.id.clone(),
            partition: self.partition,
            configuration: configuration.clone(),
            metric: metric.to_string(),
            value,
            exposure_event: seq,
        })
    }

    fn append(&mut self, configuration: ConfigurationId, kind: ExposureKind) -> usize {
        let seq = self.exposure.len();
        self.exposure.push(ExposureEvent {
            seq,
            configuration,
            kind,
        });
        seq
    }
}

/// Every partition instance in one place, with lineage resolved against the search space.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HoldoutLedger {
    holdouts: BTreeMap<HoldoutId, Holdout>,
}

impl HoldoutLedger {
    pub fn new() -> Self {
        HoldoutLedger::default()
    }

    pub fn register(&mut self, holdout: Holdout) -> Result<(), HoldoutError> {
        if self.holdouts.contains_key(&holdout.id) {
            return Err(HoldoutError::DuplicateHoldout(holdout.id.to_string()));
        }
        self.holdouts.insert(holdout.id.clone(), holdout);
        Ok(())
    }

    pub fn get(&self, id: &HoldoutId) -> Option<&Holdout> {
        self.holdouts.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Holdout> {
        self.holdouts.values()
    }

    pub fn ids(&self) -> Vec<HoldoutId> {
        self.holdouts.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.holdouts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.holdouts.is_empty()
    }

    pub(crate) fn get_mut(&mut self, id: &HoldoutId) -> Option<&mut Holdout> {
        self.holdouts.get_mut(id)
    }

    /// Scores `configuration` on `holdout`, resolving its lineage from `space` first.
    ///
    /// The lineage lookup is not a convenience. A caller who assembles the ancestor list by hand
    /// can leave an ancestor out, and the ancestor left out will be the burned one.
    pub fn measure(
        &mut self,
        holdout: &HoldoutId,
        space: &ArchitectureSpace,
        configuration: &ConfigurationId,
        metric: &str,
        value: f64,
    ) -> Result<CleanMeasurement, HoldoutError> {
        let lineage = space
            .lineage(configuration)
            .map_err(|_| HoldoutError::UnknownHoldout(configuration.to_string()))?;
        self.holdouts
            .get_mut(holdout)
            .ok_or_else(|| HoldoutError::UnknownHoldout(holdout.to_string()))?
            .measure(&lineage, metric, value)
    }

    /// Records that `configuration` was selected using `holdout`.
    pub fn record_selection(
        &mut self,
        holdout: &HoldoutId,
        configuration: &ConfigurationId,
        rationale: &str,
    ) -> Result<(), HoldoutError> {
        self.holdouts
            .get_mut(holdout)
            .ok_or_else(|| HoldoutError::UnknownHoldout(holdout.to_string()))?
            .record_selection(configuration, rationale);
        Ok(())
    }

    /// Records that a search compared `cohort` against `holdout`.
    pub fn record_search(
        &mut self,
        holdout: &HoldoutId,
        cohort: &[ConfigurationId],
    ) -> Result<(), HoldoutError> {
        self.holdouts
            .get_mut(holdout)
            .ok_or_else(|| HoldoutError::UnknownHoldout(holdout.to_string()))?
            .record_search(cohort);
        Ok(())
    }

    pub fn watermarks(&self) -> BTreeMap<HoldoutId, ExposureWatermark> {
        self.holdouts
            .iter()
            .map(|(id, holdout)| (id.clone(), holdout.watermark()))
            .collect()
    }

    /// Partitions that are still able to certify anything, with their remaining budget.
    pub fn remaining_certification_budget(&self) -> Vec<(HoldoutId, u32)> {
        self.holdouts
            .values()
            .filter(|holdout| holdout.partition.certifies() && !holdout.is_retired())
            .map(|holdout| {
                (
                    holdout.id.clone(),
                    holdout.query_budget - holdout.queries_used(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::{CandidateArchitecture, ComponentKind, ComponentSpec};

    fn minimal(id: &str) -> CandidateArchitecture {
        CandidateArchitecture::new(id)
            .with_component(ComponentSpec::new("select", ComponentKind::ContextSelector))
            .with_component(ComponentSpec::new("run", ComponentKind::Executor))
            .with_component(ComponentSpec::new("stop", ComponentKind::Terminator))
    }

    fn certifying() -> Holdout {
        Holdout::new("private-a", Partition::RotatingPrivateCertification, 8)
    }

    #[test]
    fn a_score_on_a_holdout_the_configuration_was_selected_with_is_not_a_measurement() {
        let mut holdout = certifying();
        let config = ConfigurationId::new("v2");
        holdout.record_selection(&config, "won the development panel");
        let error = holdout
            .measure(std::slice::from_ref(&config), "admissible_rate", 0.91)
            .unwrap_err();
        assert!(matches!(
            error,
            HoldoutError::SelectedUsingThisHoldout { event: 0, .. }
        ));
    }

    #[test]
    fn a_score_on_a_holdout_an_ancestor_was_selected_with_is_not_a_measurement_either() {
        let mut space = ArchitectureSpace::new();
        space.register(minimal("v1")).unwrap();
        space.register(minimal("v2").derived_from("v1")).unwrap();
        let mut ledger = HoldoutLedger::new();
        ledger.register(certifying()).unwrap();
        let holdout = HoldoutId::new("private-a");
        ledger
            .record_selection(&holdout, &ConfigurationId::new("v1"), "tuned here")
            .unwrap();

        let error = ledger
            .measure(
                &holdout,
                &space,
                &ConfigurationId::new("v2"),
                "admissible_rate",
                0.94,
            )
            .unwrap_err();
        assert_eq!(
            error,
            HoldoutError::AncestorExposed {
                holdout: "private-a".to_string(),
                configuration: "v2".to_string(),
                ancestor: "v1".to_string(),
            }
        );
    }

    #[test]
    fn measuring_a_baseline_does_not_burn_the_holdout_for_its_descendants() {
        let mut space = ArchitectureSpace::new();
        space.register(minimal("v1")).unwrap();
        space.register(minimal("v2").derived_from("v1")).unwrap();
        let mut ledger = HoldoutLedger::new();
        ledger.register(certifying()).unwrap();
        let holdout = HoldoutId::new("private-a");
        ledger
            .measure(&holdout, &space, &ConfigurationId::new("v1"), "rate", 0.70)
            .unwrap();
        let after = ledger
            .measure(&holdout, &space, &ConfigurationId::new("v2"), "rate", 0.83)
            .unwrap();
        assert_eq!(after.value(), 0.83);
    }

    #[test]
    fn selecting_propagates_to_descendants_where_merely_measuring_does_not() {
        assert!(ExposureKind::Selection {
            rationale: "won".to_string()
        }
        .propagates_to_descendants());
        assert!(ExposureKind::Search { cohort_size: 3 }.propagates_to_descendants());
        assert!(!ExposureKind::Measurement {
            metric: "rate".to_string()
        }
        .propagates_to_descendants());
    }

    #[test]
    fn a_renamed_configuration_does_not_launder_a_burned_holdout() {
        let mut space = ArchitectureSpace::new();
        space.register(minimal("v1")).unwrap();
        space.register(minimal("v1-renamed").derived_from("v1")).unwrap();
        let mut ledger = HoldoutLedger::new();
        ledger.register(certifying()).unwrap();
        let holdout = HoldoutId::new("private-a");
        ledger
            .record_selection(&holdout, &ConfigurationId::new("v1"), "tuned here")
            .unwrap();
        assert!(ledger
            .measure(
                &holdout,
                &space,
                &ConfigurationId::new("v1-renamed"),
                "admissible_rate",
                0.94
            )
            .is_err());
    }

    #[test]
    fn losing_a_search_burns_the_holdout_as_thoroughly_as_winning_it() {
        let mut holdout = certifying();
        let winner = ConfigurationId::new("v2");
        let loser = ConfigurationId::new("v3");
        holdout.record_search(&[winner, loser.clone()]);
        assert!(matches!(
            holdout.measure(&[loser], "admissible_rate", 0.4),
            Err(HoldoutError::SelectedUsingThisHoldout { .. })
        ));
    }

    #[test]
    fn a_first_query_on_an_untouched_certification_surface_is_clean() {
        let mut holdout = certifying();
        let config = ConfigurationId::new("v2");
        let measurement = holdout
            .measure(std::slice::from_ref(&config), "admissible_rate", 0.91)
            .unwrap();
        assert_eq!(measurement.value(), 0.91);
        assert_eq!(measurement.configuration(), &config);
        assert_eq!(measurement.exposure_event(), 0);
    }

    #[test]
    fn measuring_the_same_configuration_twice_is_a_repeat_query_not_a_second_measurement() {
        let mut holdout = certifying();
        let config = ConfigurationId::new("v2");
        holdout
            .measure(std::slice::from_ref(&config), "admissible_rate", 0.91)
            .unwrap();
        assert_eq!(
            holdout.measure(std::slice::from_ref(&config), "admissible_rate", 0.91),
            Err(HoldoutError::AlreadyQueried {
                holdout: "private-a".to_string(),
                configuration: "v2".to_string(),
            })
        );
    }

    #[test]
    fn a_development_panel_never_certifies_even_on_its_very_first_query() {
        let mut panel = Holdout::new("dev", Partition::ArchitectureDevelopment, 1_000);
        assert_eq!(
            panel.measure(&[ConfigurationId::new("v1")], "admissible_rate", 0.99),
            Err(HoldoutError::NotACertificationSurface {
                partition: "architecture_development".to_string(),
            })
        );
    }

    #[test]
    fn production_incidents_are_not_a_certification_surface() {
        assert!(!Partition::ProductionIncidents.certifies());
    }

    #[test]
    fn repeated_optimization_retires_the_set_as_a_true_holdout() {
        let mut holdout = Holdout::new("public", Partition::PublicEvaluation, 2);
        holdout.record_selection(&ConfigurationId::new("a"), "sweep");
        holdout.record_selection(&ConfigurationId::new("b"), "sweep");
        assert!(holdout.is_retired());
        assert_eq!(
            holdout.measure(&[ConfigurationId::new("fresh")], "rate", 0.5),
            Err(HoldoutError::Retired {
                holdout: "public".to_string(),
                used: 2,
                budget: 2,
            })
        );
    }

    #[test]
    fn a_rollback_event_does_not_consume_the_query_budget() {
        let mut holdout = Holdout::new("public", Partition::PublicEvaluation, 1);
        holdout.record_rollback(&ConfigurationId::new("v1"));
        holdout.record_rollback(&ConfigurationId::new("v1"));
        assert_eq!(holdout.queries_used(), 0);
        assert!(!holdout.is_retired());
    }

    #[test]
    fn a_rollback_event_does_not_un_burn_the_configuration_it_restores() {
        let mut holdout = certifying();
        let config = ConfigurationId::new("v1");
        holdout.record_selection(&config, "chosen here");
        holdout.record_rollback(&config);
        assert!(holdout.is_burned_for(&config));
        assert!(holdout
            .measure(std::slice::from_ref(&config), "rate", 0.7)
            .is_err());
    }

    #[test]
    fn a_non_finite_score_is_refused_rather_than_recorded_as_a_measurement() {
        let mut holdout = certifying();
        assert!(matches!(
            holdout.measure(&[ConfigurationId::new("v1")], "rate", f64::NAN),
            Err(HoldoutError::NonFiniteValue { .. })
        ));
        assert!(holdout.exposure().is_empty());
    }

    #[test]
    fn a_delta_across_two_different_holdouts_is_refused() {
        let mut first = Holdout::new("a", Partition::RotatingPrivateCertification, 4);
        let mut second = Holdout::new("b", Partition::RotatingPrivateCertification, 4);
        let before = first
            .measure(&[ConfigurationId::new("v1")], "rate", 0.5)
            .unwrap();
        let after = second
            .measure(&[ConfigurationId::new("v2")], "rate", 0.7)
            .unwrap();
        assert!(matches!(
            before.delta_to(&after),
            Err(HoldoutError::SurfaceMismatch { .. })
        ));
    }

    #[test]
    fn a_delta_across_two_different_metrics_is_refused() {
        let mut holdout = certifying();
        let before = holdout
            .measure(&[ConfigurationId::new("v1")], "admissible_rate", 0.5)
            .unwrap();
        let after = holdout
            .measure(&[ConfigurationId::new("v2")], "latency_units", 0.7)
            .unwrap();
        assert!(matches!(
            before.delta_to(&after),
            Err(HoldoutError::MetricMismatch { .. })
        ));
    }

    #[test]
    fn the_exposure_ledger_has_no_public_path_that_removes_an_event() {
        let mut holdout = certifying();
        holdout.record_selection(&ConfigurationId::new("v1"), "sweep");
        let before = holdout.exposure().len();
        holdout.record_rollback(&ConfigurationId::new("v1"));
        assert!(holdout.exposure().len() > before);
    }
}
