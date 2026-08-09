//! Criteria-aware longitudinal response, post-treatment change and uncertainty.
//!
//! Blueprint 30.06 (longitudinal response, criteria-aware) and 30.07 (post-treatment change,
//! progression and uncertainty). This is the module the rest of the crate exists to protect.
//!
//! # The one thing this module must never do
//!
//! Enhancement that increases inside the window following definitive therapy is *not* evidence
//! of progression. It is evidence of *change*, and change has several causes — 30.07 lists
//! "progression, treatment effect, mixed process, infection, or seizure-related change" and
//! marks the list open with "such as". A response call made inside that window, without
//! confirmation, is [`ResponseCall::NotEvaluable`]. It is never
//! [`ResponseCall::Progression`], and the type system enforces this rather than the
//! documentation: the progression variant carries a [`ConfirmedProgression`] whose constructor
//! is private to this module and runs only after the confirmation gate.
//!
//! The symmetric error is equally real and is also implemented: 30.06 names *"collapsing
//! not-evaluable into stable disease"* as a characteristic failure, so withholding a call
//! produces `NotEvaluable`, never `Stable`. Not-evaluable and stable are different claims —
//! one says the disease did not change, the other says we cannot say.
//!
//! A third rule constrains the withholding itself. 30.07 forbids *"calling pseudoprogression
//! from timing alone"*. Timing is sufficient to *withhold* a progression call and insufficient
//! to *assert* treatment effect, and those are not the same operation. Inside the window this
//! module returns not-evaluable plus a set of hypotheses it cannot exclude; it never returns
//! "this is pseudoprogression".
//!
//! # Criteria are data, not code
//!
//! 30.06 forbids hard-coding one criterion version. [`ResponseCriterion`] is a plain value with
//! its own version string, four are provided as constants, and [`assess`] takes a reference to
//! whichever the caller supplies. Every assessment records the id and version it used.
//!
//! # Not implemented
//!
//! Calibrated probabilities over the hypothesis set. 30.07 asks for a Brier score, which
//! requires per-hypothesis probabilities; this module produces a *set of hypotheses that
//! available evidence cannot exclude* and states which further evidence would discriminate
//! them, because inventing numbers to be scored against would be worse than admitting there are
//! none. Also absent: volumetric and qualitative measurement (only bidimensional is
//! implemented), adjudication state, automated segmentation, the ≥4-week confirmation that
//! RANO-style criteria additionally require for complete and partial response, and any use of
//! advanced imaging as evidence — perfusion, diffusion, spectroscopy and amino-acid PET appear
//! only as *requested* discriminating evidence, never as inputs.

use crate::error::OncoError;
use crate::status::{ObservationStatus, Observed};
use crate::worldline::{
    AcquisitionTime, ClinicalObservation, ClinicalTrend, Compartment, ImagingObservation,
};
use serde::{Deserialize, Serialize};

/// A versioned response rule.
///
/// Held as data so that 30.06's "criterion-version sensitivity" metric is computable: run the
/// same worldline through two of these and compare.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseCriterion {
    pub id: String,
    pub version: String,
    /// Which compartment the rule's measurements describe.
    pub compartment: Compartment,
    /// Fractional decrease from baseline required for partial response, e.g. `0.50`.
    pub partial_response_decrease: f64,
    /// Fractional increase over nadir required for progression, e.g. `0.25`.
    pub progression_increase: f64,
    /// Whether the criterion itself acknowledges post-treatment change.
    ///
    /// Recorded, not obeyed. This platform applies the post-treatment window regardless, and
    /// records the divergence where a criterion would have called progression — see
    /// [`ResponseAssessment::divergence_from_criterion`].
    pub recognises_post_treatment_change: bool,
    /// Minimum interval before a follow-up study can confirm progression.
    pub confirmation_interval_days: i64,
}

impl ResponseCriterion {
    /// Macdonald-style bidimensional criteria, predating the description of pseudoprogression.
    ///
    /// Included precisely because it is wrong about post-treatment change: it is the test case
    /// that shows the platform's safety rule is not delegated to the criterion.
    pub fn macdonald_1990() -> Self {
        ResponseCriterion {
            id: "macdonald".into(),
            version: "1990".into(),
            compartment: Compartment::ContrastEnhancing,
            partial_response_decrease: 0.50,
            progression_increase: 0.25,
            recognises_post_treatment_change: false,
            confirmation_interval_days: 28,
        }
    }

    /// High-grade glioma criteria that introduced the post-treatment window.
    pub fn high_grade_2010() -> Self {
        ResponseCriterion {
            id: "rano-hgg".into(),
            version: "2010".into(),
            compartment: Compartment::ContrastEnhancing,
            partial_response_decrease: 0.50,
            progression_increase: 0.25,
            recognises_post_treatment_change: true,
            confirmation_interval_days: 28,
        }
    }

    /// Low-grade glioma criteria, measured on the non-enhancing compartment.
    pub fn low_grade_2011() -> Self {
        ResponseCriterion {
            id: "rano-lgg".into(),
            version: "2011".into(),
            compartment: Compartment::T2Flair,
            partial_response_decrease: 0.50,
            progression_increase: 0.25,
            recognises_post_treatment_change: true,
            confirmation_interval_days: 28,
        }
    }

    /// Unified criteria with an explicit confirmation requirement.
    pub fn unified_2023() -> Self {
        ResponseCriterion {
            id: "rano-2.0".into(),
            version: "2023".into(),
            compartment: Compartment::ContrastEnhancing,
            partial_response_decrease: 0.50,
            progression_increase: 0.25,
            recognises_post_treatment_change: true,
            confirmation_interval_days: 28,
        }
    }
}

/// What the therapy was, which sets how long change remains ambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreatmentModality {
    Surgery,
    Radiotherapy,
    ChemoRadiotherapy,
    Chemotherapy,
    Immunotherapy,
    AntiAngiogenic,
    /// No definitive therapy has been delivered; there is nothing whose effect could be mistaken
    /// for progression.
    None,
}

impl TreatmentModality {
    /// Days after completion during which increased enhancement is not attributable.
    ///
    /// The twelve-week figure for irradiated disease and the six-month figure for checkpoint
    /// blockade are conventional, not blueprint-specified. They are the *only* magic numbers in
    /// this module and are stated here so a caller can see and disagree with them.
    pub const fn post_treatment_window_days(self) -> i64 {
        match self {
            TreatmentModality::Radiotherapy | TreatmentModality::ChemoRadiotherapy => 84,
            TreatmentModality::Immunotherapy => 180,
            TreatmentModality::Surgery
            | TreatmentModality::Chemotherapy
            | TreatmentModality::AntiAngiogenic
            | TreatmentModality::None => 0,
        }
    }

    /// Whether the therapy can reduce contrast enhancement without reducing tumour.
    ///
    /// The mirror image of post-treatment change: anti-angiogenic agents normalise
    /// blood-brain-barrier permeability, so an enhancement-based response may be pharmacologic
    /// rather than antineoplastic.
    pub const fn causes_pseudoresponse(self) -> bool {
        matches!(self, TreatmentModality::AntiAngiogenic)
    }
}

/// When definitive therapy finished, on the acquisition axis.
///
/// Treatment completion is an event in the subject's life, so it lives on the event-validity
/// clock alongside imaging acquisition. It is deliberately not expressible on the
/// agent-visibility clock: how late the agent learned about the therapy cannot change when the
/// therapy ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreatmentContext {
    pub modality: TreatmentModality,
    pub completed: AcquisitionTime,
}

/// A follow-up study offered as confirmation of progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmatoryStudy {
    pub acquired: AcquisitionTime,
    pub still_progressive: bool,
}

/// Evidence that can license a progression call inside the post-treatment window.
///
/// All fields default to unobserved, so a caller that supplies nothing gets the safe answer
/// rather than an accidental confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressionEvidence {
    /// Biopsy showing viable tumour rather than treatment effect.
    pub histopathologic_confirmation: Observed<bool>,
    /// Whether the new or growing lesion lies outside the high-dose radiation volume.
    pub outside_high_dose_radiation_field: Observed<bool>,
    /// A later study, if one has been acquired.
    pub confirmatory: Option<ConfirmatoryStudy>,
}

impl Default for ProgressionEvidence {
    fn default() -> Self {
        ProgressionEvidence {
            histopathologic_confirmation: Observed::Unobserved(ObservationStatus::NotCollected),
            outside_high_dose_radiation_field: Observed::Unobserved(ObservationStatus::NotCollected),
            confirmatory: None,
        }
    }
}

/// Why a confirmed progression is confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "basis")]
pub enum ProgressionBasis {
    /// The study was acquired after the window in which change is unattributable.
    OutsidePostTreatmentWindow { days_since_treatment_end: i64 },
    /// A later study, at least the criterion's confirmation interval away and outside the
    /// window, still showed progression.
    ConfirmedByFollowUp { interval_days: i64 },
    /// Tissue showed viable tumour.
    Histopathologic,
    /// New disease outside the high-dose radiation volume, which treatment effect does not
    /// explain.
    OutsideHighDoseRadiationField,
}

/// A capability token: proof that the confirmation gate ran and passed.
///
/// Constructed only by [`assess`]; the field is private and there is no public constructor, so
/// no caller outside this crate can fabricate one. [`crate::outcome`] requires this token to
/// record a progression event, which is what stops a not-evaluable scan becoming a data point
/// in a survival curve.
///
/// # Trust boundary
///
/// `Deserialize` is derived, so a token decoded from JSON asserts that *some* system ran the
/// gate. This crate cannot verify that claim, and does not pretend to: cryptographic binding of
/// the token to the assessment that produced it is **not implemented**. Use
/// [`ConfirmedProgression::basis`] to audit a decoded token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmedProgression {
    basis: ProgressionBasis,
    confirmed_at: AcquisitionTime,
}

impl ConfirmedProgression {
    pub const fn basis(&self) -> ProgressionBasis {
        self.basis
    }

    pub const fn confirmed_at(&self) -> AcquisitionTime {
        self.confirmed_at
    }
}

/// What the measurements alone imply, before any confirmation or context rule.
///
/// This is the radiologic reading, kept separate from the reportable call so that withholding is
/// visible rather than silent. It is never used as an outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseCategory {
    Complete,
    Partial,
    Stable,
    Progression,
}

/// Why a call was withheld.
///
/// Every variant names something a caller could act on: run the assay, wait the interval,
/// re-image with comparable parameters. None of them is a synonym for "stable".
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum NotEvaluableReason {
    /// Inside the post-treatment window with no confirmation. The headline rule of 30.07.
    PostTreatmentChangeNotExcluded {
        days_since_treatment_end: i64,
        window_days: i64,
    },
    /// A confirmatory study exists but is too close to the index study to confirm anything.
    ConfirmationIntervalNotMet {
        interval_days: i64,
        required_days: i64,
    },
    /// The criterion measures a compartment this study did not image.
    CompartmentMismatch {
        criterion: Compartment,
        observed: Compartment,
    },
    /// Acquisition parameters do not permit comparison with baseline.
    AcquisitionNotComparable,
    /// Neither study carries bidimensionally measurable disease.
    NoMeasurableDisease,
    /// Whether a new lesion appeared was not documented, so progression cannot be excluded.
    NewLesionStatusUnobserved(ObservationStatus),
    /// The measurement sits close enough to a threshold that inter-reader error flips the
    /// category. 30.06 requires exposing threshold sensitivity rather than asserting one side.
    NearThreshold {
        distance_to_threshold: f64,
        measurement_error: f64,
    },
    /// Corticosteroid dose rose between the comparator and this study, so apparent improvement
    /// is confounded.
    CorticosteroidDoseIncreased,
    /// Corticosteroid dose was not recorded, so the confound cannot be excluded.
    CorticosteroidDoseUnobserved(ObservationStatus),
    /// Enhancement fell under anti-angiogenic therapy, which reduces enhancement independently
    /// of tumour burden.
    PseudoresponseNotExcluded,
}

/// The reportable call.
///
/// `NotEvaluable` is a first-class outcome, not an error, and is structurally distinct from
/// `Stable` — 30.06 names collapsing the two as a characteristic failure.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "call")]
pub enum ResponseCall {
    Complete,
    Partial,
    Stable,
    Progression(ConfirmedProgression),
    NotEvaluable(NotEvaluableReason),
}

impl ResponseCall {
    /// The confirmation token, when and only when progression was confirmed.
    pub const fn confirmed_progression(&self) -> Option<&ConfirmedProgression> {
        match self {
            ResponseCall::Progression(confirmed) => Some(confirmed),
            _ => None,
        }
    }

    pub const fn label(&self) -> &'static str {
        match self {
            ResponseCall::Complete => "complete response",
            ResponseCall::Partial => "partial response",
            ResponseCall::Stable => "stable disease",
            ResponseCall::Progression(_) => "confirmed progression",
            ResponseCall::NotEvaluable(_) => "not evaluable",
        }
    }
}

/// One hypothesis for observed change (30.07).
///
/// The blueprint's list is explicitly open — "such as" — so [`ChangeHypothesis::Other`] exists
/// and is never produced by this module's builder. A caller extending the differential supplies
/// it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeHypothesis {
    Progression,
    TreatmentEffect,
    MixedProcess,
    Infection,
    SeizureRelatedChange,
    Other(String),
}

/// Evidence that would discriminate between hypotheses.
///
/// None of these are inputs to [`assess`]; they are the request 30.10's "prioritized evidence
/// request" pattern applied to 30.07's differential.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscriminatingEvidence {
    PerfusionMri,
    DiffusionMri,
    MrSpectroscopy,
    AminoAcidPet,
    Histopathology,
    IntervalFollowUp,
    CerebrospinalFluidStudies,
    ElectroencephalographyCorrelation,
}

/// A hypothesis the available evidence cannot exclude.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotExcludedHypothesis {
    pub hypothesis: ChangeHypothesis,
    pub discriminating_evidence: Vec<DiscriminatingEvidence>,
}

/// The differential for an observed change.
///
/// # Why there is no `most_likely`
///
/// 30.07 requires that alternatives be preserved and that mixed or non-identifiable states be
/// reported. An accessor that returned a single best hypothesis would be used, and would defeat
/// both requirements at the call site regardless of what the documentation said. The set is the
/// answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HypothesisSet {
    entries: Vec<NotExcludedHypothesis>,
}

impl HypothesisSet {
    pub fn entries(&self) -> &[NotExcludedHypothesis] {
        &self.entries
    }

    pub fn contains(&self, hypothesis: &ChangeHypothesis) -> bool {
        self.entries
            .iter()
            .any(|entry| &entry.hypothesis == hypothesis)
    }

    /// True when more than one hypothesis survives, i.e. the state is not identifiable from the
    /// evidence in hand.
    pub fn is_non_identifiable(&self) -> bool {
        self.entries.len() > 1
    }

    /// Every study that would narrow the set, deduplicated.
    pub fn evidence_requests(&self) -> Vec<DiscriminatingEvidence> {
        let mut requests: Vec<DiscriminatingEvidence> = self
            .entries
            .iter()
            .flat_map(|entry| entry.discriminating_evidence.iter().copied())
            .collect();
        requests.sort_unstable();
        requests.dedup();
        requests
    }
}

/// How close the measurement sits to each decision threshold (30.06).
///
/// Ratios are `Option` rather than infinity: a nadir of zero makes the fractional change
/// undefined, and `f64::INFINITY` is both a lie and unserialisable as JSON.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ThresholdSensitivity {
    pub current_spd_mm2: f64,
    pub baseline_spd_mm2: f64,
    pub nadir_spd_mm2: f64,
    /// Fractional change from nadir; `None` when the nadir is zero.
    pub change_from_nadir: Option<f64>,
    /// Fractional change from baseline; `None` when the baseline is zero.
    pub change_from_baseline: Option<f64>,
    /// Signed distance to the progression threshold. Negative is below it.
    pub distance_to_progression_threshold: Option<f64>,
    /// Signed distance to the partial-response threshold. Negative is short of it.
    pub distance_to_response_threshold: Option<f64>,
    /// The relative measurement error the caller declared.
    pub measurement_error: f64,
    /// Whether that error is large enough to move the measurement across a threshold.
    pub flips_within_measurement_error: bool,
}

/// A place where this platform declined to report what the criterion would have said.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CriterionDivergence {
    pub criterion_would_call: ResponseCategory,
    pub withheld_because: NotEvaluableReason,
    /// Whether the criterion itself acknowledges post-treatment change.
    ///
    /// `false` marks the interesting case: a criterion predating the description of
    /// pseudoprogression would have reported progression, and the platform did not.
    pub criterion_recognises_post_treatment_change: bool,
}

/// One criteria-aware response assessment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseAssessment {
    pub criterion_id: String,
    pub criterion_version: String,
    /// The radiologic reading, before confirmation and context rules.
    pub unconfirmed_reading: ResponseCategory,
    /// The reportable call.
    pub call: ResponseCall,
    pub sensitivity: ThresholdSensitivity,
    pub hypotheses: HypothesisSet,
    pub divergence_from_criterion: Option<CriterionDivergence>,
}

impl ResponseAssessment {
    /// True when the measurements read as progression and the platform did not report it.
    pub fn withheld_progression(&self) -> bool {
        self.unconfirmed_reading == ResponseCategory::Progression
            && self.call.confirmed_progression().is_none()
    }
}

/// Everything one assessment needs.
///
/// Note what is *not* here: any observation later than `current_acquired` other than an
/// explicitly offered [`ConfirmatoryStudy`]. 30.06 forbids using future clinical decline
/// retrospectively, and the request type is the enforcement point — there is no field in which
/// a caller could pass an outcome.
#[derive(Debug, Clone, Copy)]
pub struct ResponseRequest<'a> {
    pub criterion: &'a ResponseCriterion,
    pub baseline: &'a ImagingObservation,
    pub current: &'a ImagingObservation,
    pub current_acquired: AcquisitionTime,
    /// Smallest sum-of-products seen before this study, if any. Falls back to baseline.
    pub nadir_spd_mm2: Option<f64>,
    pub baseline_clinical: &'a ClinicalObservation,
    pub current_clinical: &'a ClinicalObservation,
    pub treatment: &'a TreatmentContext,
    pub evidence: &'a ProgressionEvidence,
    /// Relative inter-reader measurement error, e.g. `0.10`.
    pub measurement_error_fraction: f64,
}

/// Assess one timepoint against one criterion.
///
/// # Order of reasoning
///
/// 1. Compute the radiologic reading from measurements alone.
/// 2. Refuse the study outright where it cannot support any call — wrong compartment,
///    non-comparable acquisition, no measurable disease, undocumented new-lesion status.
/// 3. Refuse where the measurement sits within measurement error of a threshold.
/// 4. If the reading is progression, run the confirmation gate. This step is the only path to a
///    [`ConfirmedProgression`].
/// 5. If the reading is response, apply the confounds that make apparent improvement
///    uninterpretable — rising corticosteroids, anti-angiogenic therapy.
///
/// Steps 2, 3 and 5 all produce `NotEvaluable`, and step 4 produces `NotEvaluable` on failure.
/// No step produces `Stable` except a genuine stable reading.
pub fn assess(request: &ResponseRequest<'_>) -> Result<ResponseAssessment, OncoError> {
    if !request.measurement_error_fraction.is_finite()
        || request.measurement_error_fraction < 0.0
    {
        return Err(OncoError::InvalidMeasurement {
            field: "measurement_error_fraction",
            value: request.measurement_error_fraction,
        });
    }

    let baseline_spd = request.baseline.spd_mm2();
    let current_spd = request.current.spd_mm2();
    let nadir_spd = match request.nadir_spd_mm2 {
        Some(value) => {
            if !value.is_finite() || value < 0.0 {
                return Err(OncoError::InvalidMeasurement {
                    field: "nadir_spd_mm2",
                    value,
                });
            }
            value.min(baseline_spd)
        }
        None => baseline_spd,
    };

    let reading = read(request, current_spd, baseline_spd, nadir_spd);
    let sensitivity = sensitivity(request, current_spd, baseline_spd, nadir_spd);
    let hypotheses = differential(request, reading);
    let call = decide(request, reading, &sensitivity);

    let divergence = if reading_of(&call) == Some(reading) {
        None
    } else if let ResponseCall::NotEvaluable(reason) = call {
        Some(CriterionDivergence {
            criterion_would_call: reading,
            withheld_because: reason,
            criterion_recognises_post_treatment_change: request
                .criterion
                .recognises_post_treatment_change,
        })
    } else {
        None
    };

    Ok(ResponseAssessment {
        criterion_id: request.criterion.id.clone(),
        criterion_version: request.criterion.version.clone(),
        unconfirmed_reading: reading,
        call,
        sensitivity,
        hypotheses,
        divergence_from_criterion: divergence,
    })
}

const fn reading_of(call: &ResponseCall) -> Option<ResponseCategory> {
    match call {
        ResponseCall::Complete => Some(ResponseCategory::Complete),
        ResponseCall::Partial => Some(ResponseCategory::Partial),
        ResponseCall::Stable => Some(ResponseCategory::Stable),
        ResponseCall::Progression(_) => Some(ResponseCategory::Progression),
        ResponseCall::NotEvaluable(_) => None,
    }
}

/// The radiologic reading, from measurements only.
///
/// Clinical deterioration is deliberately absent. 30.07 forbids binary claims from one weak
/// signal, and a subject can decline for reasons — seizure, infection, steroid taper — that have
/// nothing to do with tumour burden. Deterioration enters the differential instead.
fn read(
    request: &ResponseRequest<'_>,
    current_spd: f64,
    baseline_spd: f64,
    nadir_spd: f64,
) -> ResponseCategory {
    if request.current.new_lesion == Observed::Value(true) {
        return ResponseCategory::Progression;
    }
    if request.current.nonmeasurable_change == Observed::Value(crate::worldline::DirectionOfChange::Increased) {
        return ResponseCategory::Progression;
    }
    if nadir_spd > 0.0 {
        if (current_spd - nadir_spd) / nadir_spd >= request.criterion.progression_increase {
            return ResponseCategory::Progression;
        }
    } else if current_spd > 0.0 {
        return ResponseCategory::Progression;
    }
    if current_spd == 0.0 {
        return ResponseCategory::Complete;
    }
    if baseline_spd > 0.0
        && (baseline_spd - current_spd) / baseline_spd >= request.criterion.partial_response_decrease
    {
        return ResponseCategory::Partial;
    }
    ResponseCategory::Stable
}

fn sensitivity(
    request: &ResponseRequest<'_>,
    current_spd: f64,
    baseline_spd: f64,
    nadir_spd: f64,
) -> ThresholdSensitivity {
    let change_from_nadir = ratio(current_spd - nadir_spd, nadir_spd);
    let change_from_baseline = ratio(current_spd - baseline_spd, baseline_spd);
    let to_progression =
        change_from_nadir.map(|change| change - request.criterion.progression_increase);
    let to_response =
        change_from_baseline.map(|change| -change - request.criterion.partial_response_decrease);
    let error = request.measurement_error_fraction;
    let flips = [to_progression, to_response]
        .into_iter()
        .flatten()
        .any(|distance| distance.abs() <= error);

    ThresholdSensitivity {
        current_spd_mm2: current_spd,
        baseline_spd_mm2: baseline_spd,
        nadir_spd_mm2: nadir_spd,
        change_from_nadir,
        change_from_baseline,
        distance_to_progression_threshold: to_progression,
        distance_to_response_threshold: to_response,
        measurement_error: error,
        flips_within_measurement_error: flips,
    }
}

fn ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if denominator > 0.0 {
        Some(numerator / denominator)
    } else {
        None
    }
}

fn decide(
    request: &ResponseRequest<'_>,
    reading: ResponseCategory,
    sensitivity: &ThresholdSensitivity,
) -> ResponseCall {
    if request.current.compartment != request.criterion.compartment {
        return ResponseCall::NotEvaluable(NotEvaluableReason::CompartmentMismatch {
            criterion: request.criterion.compartment,
            observed: request.current.compartment,
        });
    }
    if !request.current.comparable_to_baseline {
        return ResponseCall::NotEvaluable(NotEvaluableReason::AcquisitionNotComparable);
    }
    if !request.baseline.has_measurable_disease() && !request.current.has_measurable_disease() {
        return ResponseCall::NotEvaluable(NotEvaluableReason::NoMeasurableDisease);
    }
    if let Observed::Unobserved(status) = request.current.new_lesion {
        return ResponseCall::NotEvaluable(NotEvaluableReason::NewLesionStatusUnobserved(status));
    }
    if sensitivity.flips_within_measurement_error {
        let distance = [
            sensitivity.distance_to_progression_threshold,
            sensitivity.distance_to_response_threshold,
        ]
        .into_iter()
        .flatten()
        .min_by(|a, b| a.abs().total_cmp(&b.abs()))
        .unwrap_or(0.0);
        return ResponseCall::NotEvaluable(NotEvaluableReason::NearThreshold {
            distance_to_threshold: distance,
            measurement_error: sensitivity.measurement_error,
        });
    }

    match reading {
        ResponseCategory::Progression => confirm(request),
        ResponseCategory::Complete | ResponseCategory::Partial => {
            if let Some(reason) = response_confound(request) {
                ResponseCall::NotEvaluable(reason)
            } else if reading == ResponseCategory::Complete {
                ResponseCall::Complete
            } else {
                ResponseCall::Partial
            }
        }
        ResponseCategory::Stable => ResponseCall::Stable,
    }
}

/// The confirmation gate. The only constructor of [`ConfirmedProgression`].
fn confirm(request: &ResponseRequest<'_>) -> ResponseCall {
    let window = request.treatment.modality.post_treatment_window_days();
    let days_since = request
        .treatment
        .completed
        .days_until(request.current_acquired);

    if window == 0 || days_since >= window {
        return ResponseCall::Progression(ConfirmedProgression {
            basis: ProgressionBasis::OutsidePostTreatmentWindow {
                days_since_treatment_end: days_since,
            },
            confirmed_at: request.current_acquired,
        });
    }

    if request.evidence.histopathologic_confirmation == Observed::Value(true) {
        return ResponseCall::Progression(ConfirmedProgression {
            basis: ProgressionBasis::Histopathologic,
            confirmed_at: request.current_acquired,
        });
    }

    if request.evidence.outside_high_dose_radiation_field == Observed::Value(true)
        && request.current.new_lesion == Observed::Value(true)
    {
        return ResponseCall::Progression(ConfirmedProgression {
            basis: ProgressionBasis::OutsideHighDoseRadiationField,
            confirmed_at: request.current_acquired,
        });
    }

    if let Some(study) = request.evidence.confirmatory {
        let interval = request.current_acquired.days_until(study.acquired);
        let confirmatory_days_since = request.treatment.completed.days_until(study.acquired);
        if study.still_progressive
            && interval >= request.criterion.confirmation_interval_days
            && confirmatory_days_since >= window
        {
            return ResponseCall::Progression(ConfirmedProgression {
                basis: ProgressionBasis::ConfirmedByFollowUp {
                    interval_days: interval,
                },
                confirmed_at: study.acquired,
            });
        }
        if study.still_progressive && interval < request.criterion.confirmation_interval_days {
            return ResponseCall::NotEvaluable(NotEvaluableReason::ConfirmationIntervalNotMet {
                interval_days: interval,
                required_days: request.criterion.confirmation_interval_days,
            });
        }
    }

    ResponseCall::NotEvaluable(NotEvaluableReason::PostTreatmentChangeNotExcluded {
        days_since_treatment_end: days_since,
        window_days: window,
    })
}

fn response_confound(request: &ResponseRequest<'_>) -> Option<NotEvaluableReason> {
    match (
        request
            .baseline_clinical
            .corticosteroid_dexamethasone_equivalent_mg_per_day,
        request
            .current_clinical
            .corticosteroid_dexamethasone_equivalent_mg_per_day,
    ) {
        (Observed::Value(before), Observed::Value(now)) if now > before => {
            return Some(NotEvaluableReason::CorticosteroidDoseIncreased);
        }
        (Observed::Unobserved(status), _) | (_, Observed::Unobserved(status)) => {
            return Some(NotEvaluableReason::CorticosteroidDoseUnobserved(status));
        }
        _ => {}
    }

    if request.treatment.modality.causes_pseudoresponse()
        && request.current.compartment == Compartment::ContrastEnhancing
    {
        return Some(NotEvaluableReason::PseudoresponseNotExcluded);
    }
    None
}

/// Build the set of hypotheses the available evidence cannot exclude (30.07).
///
/// Timing widens the set; it never narrows it to treatment effect. Inside the window the set
/// contains progression *and* treatment effect *and* a mixed process, which is precisely the
/// statement that timing alone does not identify the state.
fn differential(request: &ResponseRequest<'_>, reading: ResponseCategory) -> HypothesisSet {
    if reading != ResponseCategory::Progression {
        return HypothesisSet::default();
    }

    let window = request.treatment.modality.post_treatment_window_days();
    let days_since = request
        .treatment
        .completed
        .days_until(request.current_acquired);
    let inside_window = window > 0 && days_since < window;

    let mut entries = vec![NotExcludedHypothesis {
        hypothesis: ChangeHypothesis::Progression,
        discriminating_evidence: vec![
            DiscriminatingEvidence::Histopathology,
            DiscriminatingEvidence::IntervalFollowUp,
        ],
    }];

    if inside_window {
        entries.push(NotExcludedHypothesis {
            hypothesis: ChangeHypothesis::TreatmentEffect,
            discriminating_evidence: vec![
                DiscriminatingEvidence::PerfusionMri,
                DiscriminatingEvidence::DiffusionMri,
                DiscriminatingEvidence::AminoAcidPet,
                DiscriminatingEvidence::Histopathology,
                DiscriminatingEvidence::IntervalFollowUp,
            ],
        });
        entries.push(NotExcludedHypothesis {
            hypothesis: ChangeHypothesis::MixedProcess,
            discriminating_evidence: vec![
                DiscriminatingEvidence::MrSpectroscopy,
                DiscriminatingEvidence::Histopathology,
            ],
        });
    }

    if request.current_clinical.trend == Observed::Value(ClinicalTrend::Deteriorated) {
        entries.push(NotExcludedHypothesis {
            hypothesis: ChangeHypothesis::Infection,
            discriminating_evidence: vec![
                DiscriminatingEvidence::DiffusionMri,
                DiscriminatingEvidence::CerebrospinalFluidStudies,
            ],
        });
        entries.push(NotExcludedHypothesis {
            hypothesis: ChangeHypothesis::SeizureRelatedChange,
            discriminating_evidence: vec![
                DiscriminatingEvidence::ElectroencephalographyCorrelation,
                DiscriminatingEvidence::DiffusionMri,
            ],
        });
    }

    HypothesisSet { entries }
}
