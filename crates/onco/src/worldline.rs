//! The longitudinal tumour worldline.
//!
//! Blueprint 30.02. A worldline is one subject's ordered stream of observations, carrying four
//! separate time axes. The blueprint states them verbatim as "event validity, recording,
//! release, and agent-visibility times", and the whole module exists to stop them collapsing
//! into one another.
//!
//! # Why four clocks and not one
//!
//! 30.02 names two failures that a single timestamp makes undetectable:
//!
//! * *sorting by record date and treating it as biological order* — a scan performed in March
//!   and transcribed in July sorts after a scan performed in May, which reverses the disease
//!   trajectory;
//! * *retrospective leakage* — evaluating what a system could have concluded at a decision time
//!   using evidence that had not yet been released to it.
//!
//! Each axis is a distinct newtype with no conversion between them, so a duration can only be
//! taken between two stamps on the *same* axis. [`AcquisitionTime::days_until`] accepts an
//! `AcquisitionTime`; there is no signature anywhere in this crate that subtracts a record time
//! from an acquisition time, because that quantity is a reporting lag and is meaningless as an
//! interval on the disease.
//!
//! # Not implemented
//!
//! Date uncertainty. 30.02 requires "uncertainty in ambiguous dates" as a metric and never says
//! how to represent it; every stamp here is exact. Also absent: event-type taxonomy beyond the
//! three observation kinds below, treatment interruption and dose summaries, and the
//! specimen/aliquot lineage 30.02 lists in required state.

use crate::error::OncoError;
use crate::status::Observed;
use crate::taxonomy::MarkerPanel;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! clock_axis {
    ($(#[$meta:meta])* $name:ident, $axis:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Timestamp);

        impl $name {
            /// The blueprint's name for this axis, used in clock-order diagnostics.
            pub const AXIS: &str = $axis;

            pub const fn new(at: Timestamp) -> Self {
                $name(at)
            }

            pub const fn timestamp(self) -> Timestamp {
                self.0
            }

            /// Whole days from `self` to `later`, on this axis only.
            ///
            /// Negative when `later` precedes `self`. Floored, so a 23-hour gap is zero days.
            pub fn days_until(self, later: Self) -> i64 {
                let nanos = later.0.as_nanos_utc() - self.0.as_nanos_utc();
                nanos.div_euclid(86_400_000_000_000) as i64
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0.to_rfc3339())
            }
        }
    };
}

clock_axis!(
    /// When the observation happened in the subject's life. The only axis on which durations
    /// have biological meaning.
    AcquisitionTime,
    "event validity"
);
clock_axis!(
    /// When the observation entered a record system.
    RecordTime,
    "recording"
);
clock_axis!(
    /// When the record was released by its custodian.
    ReleaseTime,
    "release"
);
clock_axis!(
    /// When the record became visible to an evaluated agent.
    ///
    /// This, and only this, is the axis the temporal firewall of 30.30 cuts on.
    AvailabilityTime,
    "agent visibility"
);

/// Whole days from a worldline's baseline, on the acquisition axis.
///
/// Signed: observations before the baseline are ordinary (a pre-operative scan, a prior
/// outside-institution study) and must not be clamped to zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DaysFromBaseline(i64);

impl DaysFromBaseline {
    pub const fn days(self) -> i64 {
        self.0
    }
}

impl fmt::Display for DaysFromBaseline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:+} d", self.0)
    }
}

/// The four stamps a timepoint carries.
///
/// Ordering between the axes is checked by [`Timepoint::new`], which is the only path by which
/// a `Clocks` reaches a worldline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clocks {
    pub acquired: AcquisitionTime,
    pub recorded: RecordTime,
    pub released: ReleaseTime,
    pub visible: AvailabilityTime,
}

/// A study pseudonym.
///
/// 30.30 forbids controlled or identifiable data in research outputs, so this is a study
/// identifier and never a medical record number, name, or date of birth. The type cannot
/// enforce that — it validates only that the value is a usable key — and callers are on notice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SubjectRef(String);

impl SubjectRef {
    pub fn new(value: impl Into<String>) -> Result<Self, OncoError> {
        let value = value.into();
        if value.is_empty() || value.chars().any(char::is_control) {
            return Err(OncoError::MalformedSubjectRef);
        }
        Ok(SubjectRef(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SubjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<SubjectRef> for String {
    fn from(value: SubjectRef) -> Self {
        value.0
    }
}

impl TryFrom<String> for SubjectRef {
    type Error = OncoError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SubjectRef::new(value)
    }
}

/// Which compartment a lesion measurement describes.
///
/// Response criteria are compartment-specific: a rule written for contrast-enhancing tumour
/// applied to a T2/FLAIR measurement produces a number with no defined meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Compartment {
    ContrastEnhancing,
    T2Flair,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImagingModality {
    MriT1PostContrast,
    MriT2Flair,
    MriPerfusion,
    MriDiffusion,
    AminoAcidPet,
}

/// Direction of change in disease that is present but not measurable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionOfChange {
    Increased,
    Unchanged,
    Decreased,
}

/// Clinical trajectory between two timepoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClinicalTrend {
    Improved,
    Stable,
    Deteriorated,
}

/// Karnofsky performance status, a decile scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8")]
pub struct Karnofsky(u8);

impl Karnofsky {
    pub fn new(value: u8) -> Result<Self, OncoError> {
        if value > 100 || value % 10 != 0 {
            return Err(OncoError::InvalidKarnofsky(value));
        }
        Ok(Karnofsky(value))
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Karnofsky {
    type Error = OncoError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Karnofsky::new(value)
    }
}

/// One bidimensionally measured lesion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TargetLesionDocument", into = "TargetLesionDocument")]
pub struct TargetLesion {
    label: String,
    longest_diameter_mm: f64,
    perpendicular_diameter_mm: f64,
}

impl TargetLesion {
    pub fn new(
        label: impl Into<String>,
        longest_diameter_mm: f64,
        perpendicular_diameter_mm: f64,
    ) -> Result<Self, OncoError> {
        check_measurement("longest_diameter_mm", longest_diameter_mm)?;
        check_measurement("perpendicular_diameter_mm", perpendicular_diameter_mm)?;
        Ok(TargetLesion {
            label: label.into(),
            longest_diameter_mm,
            perpendicular_diameter_mm,
        })
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    /// Product of perpendicular diameters, the bidimensional measurement of 30.06.
    pub fn product_mm2(&self) -> f64 {
        self.longest_diameter_mm * self.perpendicular_diameter_mm
    }
}

fn check_measurement(field: &'static str, value: f64) -> Result<(), OncoError> {
    if !value.is_finite() || value < 0.0 {
        return Err(OncoError::InvalidMeasurement { field, value });
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TargetLesionDocument {
    label: String,
    longest_diameter_mm: f64,
    perpendicular_diameter_mm: f64,
}

impl TryFrom<TargetLesionDocument> for TargetLesion {
    type Error = OncoError;

    fn try_from(document: TargetLesionDocument) -> Result<Self, Self::Error> {
        TargetLesion::new(
            document.label,
            document.longest_diameter_mm,
            document.perpendicular_diameter_mm,
        )
    }
}

impl From<TargetLesion> for TargetLesionDocument {
    fn from(value: TargetLesion) -> Self {
        TargetLesionDocument {
            label: value.label,
            longest_diameter_mm: value.longest_diameter_mm,
            perpendicular_diameter_mm: value.perpendicular_diameter_mm,
        }
    }
}

/// One imaging study, reduced to what response assessment consumes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImagingObservation {
    pub modality: ImagingModality,
    pub compartment: Compartment,
    pub target_lesions: Vec<TargetLesion>,
    /// Whether a new lesion appeared.
    ///
    /// `Observed<bool>` rather than `bool`: 30.02 requires that the absence of an event be
    /// distinguished from missing documentation, and "the report does not mention new lesions"
    /// is not "there are no new lesions".
    pub new_lesion: Observed<bool>,
    /// Change in disease that is present but not bidimensionally measurable.
    pub nonmeasurable_change: Observed<DirectionOfChange>,
    /// Whether acquisition parameters permit comparison with the baseline study.
    pub comparable_to_baseline: bool,
}

impl ImagingObservation {
    /// Sum of the products of perpendicular diameters across target lesions.
    pub fn spd_mm2(&self) -> f64 {
        self.target_lesions
            .iter()
            .map(TargetLesion::product_mm2)
            .sum()
    }

    pub fn has_measurable_disease(&self) -> bool {
        !self.target_lesions.is_empty()
    }
}

/// The clinical context a response rule needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClinicalObservation {
    pub corticosteroid_dexamethasone_equivalent_mg_per_day: Observed<f64>,
    pub performance_status: Observed<Karnofsky>,
    pub trend: Observed<ClinicalTrend>,
}

/// What a timepoint observed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Observation {
    Imaging(ImagingObservation),
    Molecular(MarkerPanel),
    Clinical(ClinicalObservation),
}

/// One observation, stamped on all four axes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TimepointDocument", into = "TimepointDocument")]
pub struct Timepoint {
    label: String,
    clocks: Clocks,
    observation: Observation,
}

impl Timepoint {
    /// Build a timepoint, checking that the four axes are consistently ordered.
    ///
    /// The order acquisition ≤ recording ≤ release ≤ agent visibility is not a convention: each
    /// step is a physical dependency. Violating it means either the source swapped two fields or
    /// evidence was back-dated, and both make every downstream temporal claim unsound.
    pub fn new(
        label: impl Into<String>,
        clocks: Clocks,
        observation: Observation,
    ) -> Result<Self, OncoError> {
        let label = label.into();
        check_order(
            &label,
            AcquisitionTime::AXIS,
            clocks.acquired.timestamp(),
            RecordTime::AXIS,
            clocks.recorded.timestamp(),
        )?;
        check_order(
            &label,
            RecordTime::AXIS,
            clocks.recorded.timestamp(),
            ReleaseTime::AXIS,
            clocks.released.timestamp(),
        )?;
        check_order(
            &label,
            ReleaseTime::AXIS,
            clocks.released.timestamp(),
            AvailabilityTime::AXIS,
            clocks.visible.timestamp(),
        )?;
        Ok(Timepoint {
            label,
            clocks,
            observation,
        })
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn clocks(&self) -> Clocks {
        self.clocks
    }

    pub const fn acquired(&self) -> AcquisitionTime {
        self.clocks.acquired
    }

    pub const fn visible(&self) -> AvailabilityTime {
        self.clocks.visible
    }

    pub const fn observation(&self) -> &Observation {
        &self.observation
    }

    pub const fn imaging(&self) -> Option<&ImagingObservation> {
        match &self.observation {
            Observation::Imaging(imaging) => Some(imaging),
            _ => None,
        }
    }

    pub const fn clinical(&self) -> Option<&ClinicalObservation> {
        match &self.observation {
            Observation::Clinical(clinical) => Some(clinical),
            _ => None,
        }
    }
}

fn check_order(
    label: &str,
    earlier_axis: &'static str,
    earlier: Timestamp,
    later_axis: &'static str,
    later: Timestamp,
) -> Result<(), OncoError> {
    if later < earlier {
        return Err(OncoError::ClockOrderViolation {
            timepoint: label.to_string(),
            earlier_axis,
            earlier,
            later_axis,
            later,
        });
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct TimepointDocument {
    label: String,
    clocks: Clocks,
    observation: Observation,
}

impl TryFrom<TimepointDocument> for Timepoint {
    type Error = OncoError;

    fn try_from(document: TimepointDocument) -> Result<Self, Self::Error> {
        Timepoint::new(document.label, document.clocks, document.observation)
    }
}

impl From<Timepoint> for TimepointDocument {
    fn from(value: Timepoint) -> Self {
        TimepointDocument {
            label: value.label,
            clocks: value.clocks,
            observation: value.observation,
        }
    }
}

/// One subject's ordered observation stream (30.02).
///
/// Held in acquisition order — biological order — at all times. A worldline always has a
/// baseline, because a worldline without one cannot answer "how long after baseline", and an
/// object that cannot answer its defining question should not be constructible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "WorldlineDocument", into = "WorldlineDocument")]
pub struct TumourWorldline {
    subject: SubjectRef,
    baseline_label: String,
    timepoints: Vec<Timepoint>,
}

impl TumourWorldline {
    pub fn new(subject: SubjectRef, baseline: Timepoint) -> Self {
        let baseline_label = baseline.label.clone();
        TumourWorldline {
            subject,
            baseline_label,
            timepoints: vec![baseline],
        }
    }

    /// Insert a timepoint, keeping acquisition order.
    ///
    /// Insertion, not append: real feeds deliver out of order, and refusing late-arriving
    /// evidence would be a worse failure than sorting it. What is refused is a duplicate label,
    /// because labels index the stream.
    pub fn push(&mut self, timepoint: Timepoint) -> Result<(), OncoError> {
        if self
            .timepoints
            .iter()
            .any(|existing| existing.label == timepoint.label)
        {
            return Err(OncoError::DuplicateTimepoint(timepoint.label));
        }
        let position = self
            .timepoints
            .partition_point(|existing| existing.acquired() <= timepoint.acquired());
        self.timepoints.insert(position, timepoint);
        Ok(())
    }

    pub const fn subject(&self) -> &SubjectRef {
        &self.subject
    }

    /// Timepoints in biological order.
    pub fn timepoints(&self) -> &[Timepoint] {
        &self.timepoints
    }

    /// Timepoints in the order a record system learned of them.
    ///
    /// Provided because reporting-lag analysis needs it, and named so that no one mistakes it
    /// for a disease trajectory. 30.02 names sorting by record date and treating it as
    /// biological order as a characteristic failure; the two orders differ whenever
    /// transcription lags vary, which is always.
    pub fn in_record_order(&self) -> Vec<&Timepoint> {
        let mut ordered: Vec<&Timepoint> = self.timepoints.iter().collect();
        ordered.sort_by_key(|timepoint| timepoint.clocks.recorded);
        ordered
    }

    pub fn baseline(&self) -> &Timepoint {
        self.timepoints
            .iter()
            .find(|timepoint| timepoint.label == self.baseline_label)
            .expect("the baseline is inserted at construction and timepoints are never removed")
    }

    /// Days from baseline, on the acquisition axis. Negative before baseline.
    pub fn time_from_baseline(&self, timepoint: &Timepoint) -> DaysFromBaseline {
        DaysFromBaseline(
            self.baseline()
                .acquired()
                .days_until(timepoint.acquired()),
        )
    }

    /// The temporal firewall (30.30 release gate, 30.02 leakage metric).
    ///
    /// Everything an agent could legitimately have seen at `cutoff`, cut on agent-visibility
    /// time. Cutting on acquisition time instead would admit scans that had been performed but
    /// not yet released, which is the leak this method exists to prevent — and is why no
    /// acquisition-time filter is offered here.
    pub fn visible_at(&self, cutoff: AvailabilityTime) -> Vec<&Timepoint> {
        self.timepoints
            .iter()
            .filter(|timepoint| timepoint.clocks.visible <= cutoff)
            .collect()
    }

    /// Imaging studies in biological order.
    pub fn imaging(&self) -> impl Iterator<Item = (&Timepoint, &ImagingObservation)> {
        self.timepoints
            .iter()
            .filter_map(|timepoint| timepoint.imaging().map(|imaging| (timepoint, imaging)))
    }

    /// Smallest sum-of-products seen strictly before `before`, among studies visible at `cutoff`.
    ///
    /// The nadir is the comparator for progression, so it inherits the firewall: a nadir
    /// computed from a scan the agent had not yet been shown makes the resulting progression
    /// call unreproducible at the decision time.
    pub fn nadir_spd_mm2(
        &self,
        compartment: Compartment,
        before: AcquisitionTime,
        cutoff: AvailabilityTime,
    ) -> Option<f64> {
        self.imaging()
            .filter(|(timepoint, imaging)| {
                timepoint.acquired() < before
                    && timepoint.clocks.visible <= cutoff
                    && imaging.compartment == compartment
                    && imaging.has_measurable_disease()
            })
            .map(|(_, imaging)| imaging.spd_mm2())
            .fold(None, |smallest: Option<f64>, spd| {
                Some(smallest.map_or(spd, |value| value.min(spd)))
            })
    }
}

#[derive(Serialize, Deserialize)]
struct WorldlineDocument {
    subject: SubjectRef,
    baseline_label: String,
    timepoints: Vec<Timepoint>,
}

impl TryFrom<WorldlineDocument> for TumourWorldline {
    type Error = OncoError;

    fn try_from(document: WorldlineDocument) -> Result<Self, Self::Error> {
        if !document
            .timepoints
            .iter()
            .any(|timepoint| timepoint.label == document.baseline_label)
        {
            return Err(OncoError::UnknownBaseline(document.baseline_label));
        }
        let mut timepoints = document.timepoints;
        timepoints.sort_by_key(Timepoint::acquired);
        Ok(TumourWorldline {
            subject: document.subject,
            baseline_label: document.baseline_label,
            timepoints,
        })
    }
}

impl From<TumourWorldline> for WorldlineDocument {
    fn from(value: TumourWorldline) -> Self {
        WorldlineDocument {
            subject: value.subject,
            baseline_label: value.baseline_label,
            timepoints: value.timepoints,
        }
    }
}
