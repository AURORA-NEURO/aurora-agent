//! Grading against a reference standard that is itself uncertain (31.01, 26.02, 26.04).
//!
//! The move this module refuses to make is the one every evaluation harness makes by default:
//! turning "the prediction matched the reference" into a number, and then forgetting that the
//! reference was 60% confident. Once that number exists, it is averaged, ranked and published,
//! and the reference's uncertainty is gone for good.
//!
//! # The two readings of a spread
//!
//! A reference that says `0.55 progression / 0.35 treatment-effect / 0.10 mixed` supports exactly
//! two defensible scorings, and they disagree:
//!
//! * **If the spread is irreducible biology** — 31.01's aleatoric case, a spatially mixed tumour
//!   that really is two things — then the reference distribution *is* the target, and the right
//!   score is a proper scoring rule against it. A calibrated forecast matching `0.55/0.35/0.10`
//!   is perfect. An unqualified categorical label is not, because it asserted certainty the
//!   biology does not support.
//! * **If the spread is annotation error** — readers who disagreed because the rubric was vague,
//!   not because the tumour was mixed — then the reference's mode is the truth and the spread is
//!   noise in the instrument. Now the categorical label is perfect and the hedged forecast is
//!   the one being punished for the benchmark's defect.
//!
//! Both are computed. [`ScoreInterval`] carries both ends, and the width of the interval *is* the
//! reference's uncertainty about this case, expressed in the units of the score. A resolved
//! reference gives a degenerate interval; that is the only shape [`BioScore::is_clean_pass`]
//! accepts, which is how "agreeing with a 60%-confident reference" is prevented from reading as a
//! pass.
//!
//! # Why there is no `fn value(&self) -> f64`
//!
//! Collapsing the interval requires deciding which reading is right, and that decision belongs to
//! the reference standard's own dispersion attribution, not to whoever is drawing the chart. So
//! the only route to a scalar is [`BioScore::collapse`], which takes a named [`CollapsePolicy`]
//! and fails — with [`crate::error::CollapseError`] — when the policy's discharge contradicts the
//! declared dispersion or when nobody attributed the dispersion at all. A score that ignores
//! reference uncertainty is not hard to obtain here; it is unobtainable.
//!
//! # Not implemented
//!
//! The scoring rule is Brier only. The logarithmic rule is deliberately absent: it is unbounded
//! on a state the prediction gave zero mass, and an unbounded penalty on one case dominates any
//! aggregate, which makes it a poor instrument for a benchmark where reference states are
//! themselves sometimes mis-enumerated. Ranked probability scoring over ordered outcomes — stage,
//! grade, response category — is also missing and would need the ordering that 31.01's partial
//! orders would supply.

use std::collections::BTreeMap;

use bioprism_ids::WorldId;
use bioprism_section::OracleStatus;
use serde::{Deserialize, Serialize};

use crate::comparability::{
    gate, Bridge, ComparabilityRequirement, ComparabilityWitness, MeasurementFrame,
};
use crate::credit::Credit;
use crate::error::{CollapseError, PredictionError, ScoreError};
use crate::layer::ClassifiedError;
use crate::reference::{
    Dispersion, ReferenceDistribution, ReferenceStandard, Resolution, MASS_TOLERANCE,
};
use crate::wrongness::Severity;

/// The proper scoring rule used to compare a prediction with a reference distribution.
///
/// One variant, named rather than implicit, because 26.24 requires a published result to state
/// how it was computed and "accuracy" is not a statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoringRule {
    /// `1 - Σ(pᵢ - rᵢ)² / 2`. Proper, bounded in `[0, 1]`, and 1.0 exactly when the prediction
    /// equals the reference distribution.
    Brier,
}

impl ScoringRule {
    pub fn as_str(self) -> &'static str {
        match self {
            ScoringRule::Brier => "brier",
        }
    }
}

/// A normalised forecast over reference states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredictedDistribution {
    mass: BTreeMap<String, f64>,
}

impl PredictedDistribution {
    pub fn new(mass: impl IntoIterator<Item = (String, f64)>) -> Result<Self, PredictionError> {
        let mut table: BTreeMap<String, f64> = BTreeMap::new();
        for (state, m) in mass {
            if !m.is_finite() {
                return Err(PredictionError::NonFiniteMass { state });
            }
            if m < 0.0 {
                return Err(PredictionError::NegativeMass { state, mass: m });
            }
            if table.contains_key(&state) {
                return Err(PredictionError::DuplicateState { state });
            }
            table.insert(state, m);
        }
        if table.is_empty() {
            return Err(PredictionError::NoPredictedState);
        }
        let total: f64 = table.values().sum();
        if (total - 1.0).abs() > MASS_TOLERANCE {
            return Err(PredictionError::MassNotNormalised {
                total,
                tolerance: MASS_TOLERANCE,
            });
        }
        Ok(PredictedDistribution { mass: table })
    }

    pub fn mass_on(&self, state: &str) -> f64 {
        self.mass.get(state).copied().unwrap_or(0.0)
    }

    pub fn states(&self) -> impl Iterator<Item = &str> {
        self.mass.keys().map(String::as_str)
    }
}

/// What the evaluated system said.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "prediction", rename_all = "snake_case")]
pub enum Prediction {
    /// An unqualified label. Treated as a degenerate distribution, which is what makes the
    /// comparison with a hedged forecast fair rather than rhetorical.
    Categorical { state: String },
    /// A forecast.
    Distributional(PredictedDistribution),
    /// The system declined to answer. Graded by [`Grade::Abstained`], never as a zero.
    Abstained { reason: String },
}

impl Prediction {
    pub fn categorical(state: impl Into<String>) -> Self {
        Prediction::Categorical {
            state: state.into(),
        }
    }
}

/// The shape of the prediction that produced a score, retained on the score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredictionForm {
    Categorical,
    Distributional,
}

impl PredictionForm {
    pub fn as_str(self) -> &'static str {
        match self {
            PredictionForm::Categorical => "categorical",
            PredictionForm::Distributional => "distributional",
        }
    }
}

/// The band of scores the reference's own uncertainty admits.
///
/// The two ends are not a confidence interval on sampling noise. They are the two attributions of
/// the reference's spread, each carried to its scoring consequence. When the reference is
/// resolved they coincide.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScoreInterval {
    /// Score against the reference distribution itself — the reading in which the spread is real
    /// biology and asserting certainty is an error.
    pub under_aleatoric: f64,
    /// Score against the reference's modal state alone — the reading in which the spread is
    /// annotation noise and hedging is an error.
    pub under_annotation_error: f64,
}

impl ScoreInterval {
    pub fn lo(&self) -> f64 {
        self.under_aleatoric.min(self.under_annotation_error)
    }

    pub fn hi(&self) -> f64 {
        self.under_aleatoric.max(self.under_annotation_error)
    }

    /// How much of the score is undetermined by the evidence. Zero exactly when the reference
    /// resolves the case.
    pub fn width(&self) -> f64 {
        self.hi() - self.lo()
    }

    pub fn is_point(&self) -> bool {
        self.width() <= MASS_TOLERANCE
    }
}

/// How a caller proposes to discharge the reference's uncertainty in order to get one number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceDischarge {
    /// Take the spread as irreducible and report the proper-scoring value against the full
    /// distribution. Always admissible on an attributed reference.
    Aleatoric,
    /// Take the spread as annotation noise and score against the mode. Refused when the reference
    /// declared its spread aleatoric, because that is denying the biology in order to score
    /// higher.
    AnnotationError,
    /// Interpolate between the two ends using the reference's declared aleatoric fraction.
    AsDeclared,
    /// Report the pessimistic end whatever the attribution. Cannot overstate, so it is the one
    /// discharge available on an unattributed reference — at the price of being a lower bound
    /// rather than an estimate.
    Conservative,
}

impl ReferenceDischarge {
    pub fn as_str(self) -> &'static str {
        match self {
            ReferenceDischarge::Aleatoric => "aleatoric",
            ReferenceDischarge::AnnotationError => "annotation_error",
            ReferenceDischarge::AsDeclared => "as_declared",
            ReferenceDischarge::Conservative => "conservative",
        }
    }
}

/// A named, publishable rule for turning a [`ScoreInterval`] into a scalar.
///
/// Named because 26.20 forbids retroactive weight changes, and a discharge chosen after seeing
/// the leaderboard is a retroactive weight change wearing a different hat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollapsePolicy {
    pub policy_id: String,
    pub discharge: ReferenceDischarge,
    /// Cases whose reference places less than this mass on its own modal answer are not scored at
    /// all. A benchmark built from references that cannot exceed 0.5 confidence is measuring its
    /// annotators.
    pub minimum_reference_confidence: f64,
    /// When true, even [`ReferenceDischarge::Conservative`] is refused on an unattributed
    /// reference.
    pub require_attributed_dispersion: bool,
}

impl CollapsePolicy {
    /// The default publishable policy: proper scoring against the full reference distribution,
    /// no case scored below 0.5 reference confidence, attribution required.
    pub fn strict(policy_id: impl Into<String>) -> Self {
        CollapsePolicy {
            policy_id: policy_id.into(),
            discharge: ReferenceDischarge::Aleatoric,
            minimum_reference_confidence: 0.5,
            require_attributed_dispersion: true,
        }
    }
}

/// A graded prediction.
///
/// Abstention is a separate variant rather than a low score. 31.01 is explicit that `unresolved`
/// "is not scored as a hidden negative", and the same reasoning applies to a system that declines:
/// folding it into the same numeric channel as a wrong answer makes an abstention indistinguishable
/// from a mistake in every downstream average.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "grade", rename_all = "snake_case")]
pub enum Grade {
    Scored(Box<BioScore>),
    Abstained(AbstentionRecord),
}

impl Grade {
    pub fn score(&self) -> Option<&BioScore> {
        match self {
            Grade::Scored(s) => Some(s),
            Grade::Abstained(_) => None,
        }
    }

    pub fn abstention(&self) -> Option<&AbstentionRecord> {
        match self {
            Grade::Abstained(a) => Some(a),
            Grade::Scored(_) => None,
        }
    }
}

/// A declined answer, with whether declining was the right call (26.04).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbstentionRecord {
    pub subject: String,
    pub reason: String,
    /// Whether the reference standard itself was unable to decide, or was decided but diffuse
    /// enough that abstention was defensible.
    pub warranted: bool,
    /// The reference's own confidence in its modal answer, or `None` when the reference did not
    /// produce a distribution at all.
    pub reference_modal_confidence: Option<f64>,
    pub reference_state: String,
}

/// A score that cannot be read without reading the reference's uncertainty.
///
/// The numeric fields are private. [`BioScore::interval`] returns a pair, which cannot be mistaken
/// for a score, and [`BioScore::collapse`] is the only route to a scalar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BioScore {
    subject: String,
    grader_id: String,
    requirement_id: String,
    rule: ScoringRule,
    form: PredictionForm,
    interval: ScoreInterval,
    resolution: Resolution,
    dispersion: Dispersion,
    reference_entropy_bits: f64,
    bridge_loss: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    parent_world: Option<WorldId>,
    errors: Vec<ClassifiedError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    credit: Option<Credit>,
}

impl BioScore {
    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn grader_id(&self) -> &str {
        &self.grader_id
    }

    /// The comparability requirement this score passed. A score whose requirement is not stated
    /// cannot be compared with another score.
    pub fn requirement_id(&self) -> &str {
        &self.requirement_id
    }

    pub fn rule(&self) -> ScoringRule {
        self.rule
    }

    pub fn form(&self) -> PredictionForm {
        self.form
    }

    pub fn interval(&self) -> ScoreInterval {
        self.interval
    }

    pub fn resolution(&self) -> Resolution {
        self.resolution
    }

    pub fn dispersion(&self) -> Dispersion {
        self.dispersion
    }

    pub fn reference_entropy_bits(&self) -> f64 {
        self.reference_entropy_bits
    }

    /// Declared loss from any bridge used to make the measurements comparable. Non-zero means the
    /// comparison was legitimate and lossy, and a published score should say so.
    pub fn bridge_loss(&self) -> f64 {
        self.bridge_loss
    }

    pub fn errors(&self) -> &[ClassifiedError] {
        &self.errors
    }

    pub fn credit(&self) -> Option<&Credit> {
        self.credit.as_ref()
    }

    /// The BioWorld this case was generated from, when the caller declared it.
    ///
    /// 26.02's statistical-analysis clause: "Generated descendants cannot be treated as
    /// independent observations merely because they have different identifiers." Scores that do
    /// not carry a parent cannot be checked for that, which is why
    /// [`crate::aggregate::PooledScore::effective_n`] returns `None` rather than a count when any
    /// member is undeclared.
    pub fn parent_world(&self) -> Option<&WorldId> {
        self.parent_world.as_ref()
    }

    /// Declares which BioWorld this case descends from.
    pub fn from_world(mut self, world: WorldId) -> Self {
        self.parent_world = Some(world);
        self
    }

    /// Attaches classified errors. Kept separate from grading because classification is a
    /// judgement about *how* the prediction went wrong, which the numeric comparison cannot make.
    pub fn with_errors(mut self, errors: impl IntoIterator<Item = ClassifiedError>) -> Self {
        self.errors.extend(errors);
        self
    }

    /// Attaches partial credit. The credit already carries the rule that produced it; see
    /// [`crate::credit`].
    pub fn with_credit(mut self, credit: Credit) -> Self {
        self.credit = Some(credit);
        self
    }

    pub fn worst_severity(&self) -> Option<Severity> {
        self.errors.iter().map(ClassifiedError::severity).max()
    }

    pub fn has_critical_error(&self) -> bool {
        self.worst_severity() == Some(Severity::Critical)
    }

    /// True only when the reference resolved the case *and* the prediction scored the maximum.
    ///
    /// This is the guard the whole crate exists for. Agreement with a reference that places 0.6
    /// on its own modal answer produces an interval of non-zero width, so it returns false
    /// however good the agreement was.
    pub fn is_clean_pass(&self) -> bool {
        self.resolution.is_categorical()
            && self.interval.is_point()
            && (1.0 - self.interval.lo()).abs() <= MASS_TOLERANCE
            && !self.has_critical_error()
    }

    /// Projects onto the three-valued status of `bioprism_section`.
    ///
    /// Any reference uncertainty projects to [`OracleStatus::Underdetermined`] rather than to a
    /// pass. A consumer that only understands three values must not be handed `Valid` for a case
    /// the reference could not decide.
    pub fn status(&self) -> OracleStatus {
        if self.has_critical_error() {
            return OracleStatus::Invalid;
        }
        if !self.interval.is_point() {
            return OracleStatus::Underdetermined;
        }
        if (1.0 - self.interval.lo()).abs() <= MASS_TOLERANCE {
            OracleStatus::Valid
        } else {
            OracleStatus::Invalid
        }
    }

    /// Turns the interval into one number under a named policy, or refuses.
    ///
    /// Every refusal is a case where the number would have asserted something the reference
    /// standard does not support. See [`CollapseError`].
    pub fn collapse(&self, policy: &CollapsePolicy) -> Result<f64, CollapseError> {
        let floor = policy.minimum_reference_confidence;
        if !(0.0..=1.0).contains(&floor) || !floor.is_finite() {
            return Err(CollapseError::MalformedPolicy {
                policy_id: policy.policy_id.clone(),
                floor,
            });
        }

        let modal = self.resolution.modal_mass();
        if modal + MASS_TOLERANCE < floor {
            return Err(CollapseError::ReferenceBelowPolicyFloor {
                policy_id: policy.policy_id.clone(),
                required: floor,
                available: modal,
            });
        }

        let attributed = self.dispersion.is_attributed();
        if !attributed
            && (policy.require_attributed_dispersion
                || policy.discharge != ReferenceDischarge::Conservative)
        {
            return Err(CollapseError::UnattributedDispersion {
                policy_id: policy.policy_id.clone(),
            });
        }

        match policy.discharge {
            ReferenceDischarge::Aleatoric => Ok(self.interval.under_aleatoric),
            ReferenceDischarge::AnnotationError => {
                if self.dispersion == Dispersion::Aleatoric && !self.interval.is_point() {
                    return Err(CollapseError::DischargeContradictsDispersion {
                        policy_id: policy.policy_id.clone(),
                        discharge: policy.discharge.as_str().to_string(),
                        dispersion: self.dispersion.as_str().to_string(),
                    });
                }
                Ok(self.interval.under_annotation_error)
            }
            ReferenceDischarge::AsDeclared => {
                let irreducible = self.dispersion.irreducible_fraction().ok_or_else(|| {
                    CollapseError::UnattributedDispersion {
                        policy_id: policy.policy_id.clone(),
                    }
                })?;
                Ok(self.interval.under_aleatoric * irreducible
                    + self.interval.under_annotation_error * (1.0 - irreducible))
            }
            ReferenceDischarge::Conservative => Ok(self.interval.lo()),
        }
    }
}

/// Grades predictions against reference standards under a fixed comparability requirement.
///
/// The grader is identified and the requirement is fixed at construction, so that every score it
/// emits names the gate it passed. A grader that accepts any witness is a grader whose scores
/// cannot be pooled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grader {
    grader_id: String,
    requirement: ComparabilityRequirement,
    /// Reference entropy, in bits, above which declining to answer counts as warranted.
    abstention_entropy_threshold: f64,
}

impl Grader {
    pub fn new(grader_id: impl Into<String>, requirement: ComparabilityRequirement) -> Self {
        Grader {
            grader_id: grader_id.into(),
            requirement,
            abstention_entropy_threshold: 1.0,
        }
    }

    pub fn with_abstention_threshold(mut self, bits: f64) -> Self {
        self.abstention_entropy_threshold = bits;
        self
    }

    pub fn grader_id(&self) -> &str {
        &self.grader_id
    }

    pub fn requirement(&self) -> &ComparabilityRequirement {
        &self.requirement
    }

    /// Gates two measurement frames and grades in one step.
    ///
    /// The ergonomic entry point, and the one that shows what the gate buys: an incomparable pair
    /// comes back as [`ScoreError::Incomparable`] carrying every failing dimension, not as a
    /// number with a caveat attached. Callers holding a witness already — because they are grading
    /// many predictions under one frame pair — should use [`Grader::grade`] directly.
    pub fn gate_and_grade(
        &self,
        prediction_frame: &MeasurementFrame,
        reference_frame: &MeasurementFrame,
        bridges: &[Bridge],
        subject: impl Into<String>,
        prediction: &Prediction,
        reference: &ReferenceStandard,
    ) -> Result<Grade, ScoreError> {
        let witness = gate(
            &self.requirement,
            prediction_frame,
            reference_frame,
            bridges,
        )
        .map_err(ScoreError::Incomparable)?;
        self.grade(&witness, subject, prediction, reference)
    }

    /// Grades one prediction.
    ///
    /// The [`ComparabilityWitness`] is a required argument, not an option. Obtaining one means
    /// [`crate::comparability::gate`] agreed the two measurements are on the same footing; there
    /// is no overload without it.
    pub fn grade(
        &self,
        witness: &ComparabilityWitness,
        subject: impl Into<String>,
        prediction: &Prediction,
        reference: &ReferenceStandard,
    ) -> Result<Grade, ScoreError> {
        let subject = subject.into();
        if witness.requirement_id() != self.requirement.requirement_id {
            return Err(ScoreError::WitnessFromDifferentRequirement {
                expected: self.requirement.requirement_id.clone(),
                found: witness.requirement_id().to_string(),
            });
        }

        if let Prediction::Abstained { reason } = prediction {
            return Ok(Grade::Abstained(
                self.grade_abstention(subject, reason, reference),
            ));
        }

        let distribution = match reference {
            ReferenceStandard::Distribution(d) => d,
            ReferenceStandard::Unresolved { reason } => {
                return Err(ScoreError::ReferenceUnresolved {
                    reason: reason.clone(),
                })
            }
            ReferenceStandard::NotEvaluable { reason } => {
                return Err(ScoreError::ReferenceNotEvaluable {
                    reason: reason.clone(),
                })
            }
        };

        let (form, predicted) = match prediction {
            Prediction::Categorical { state } => {
                if !distribution.admits(state) {
                    return Err(ScoreError::StateOutsideReference {
                        state: state.clone(),
                    });
                }
                (
                    PredictionForm::Categorical,
                    BTreeMap::from([(state.clone(), 1.0)]),
                )
            }
            Prediction::Distributional(d) => {
                for state in d.states() {
                    if !distribution.admits(state) {
                        return Err(ScoreError::StateOutsideReference {
                            state: state.to_string(),
                        });
                    }
                }
                (
                    PredictionForm::Distributional,
                    d.states().map(|s| (s.to_string(), d.mass_on(s))).collect(),
                )
            }
            Prediction::Abstained { .. } => unreachable!("abstention handled above"),
        };

        let modal_state = distribution.mode().0.to_string();
        let interval = ScoreInterval {
            under_aleatoric: brier_agreement(&predicted, distribution, |s| {
                distribution.mass_on(s).unwrap_or(0.0)
            }),
            under_annotation_error: brier_agreement(&predicted, distribution, |s| {
                if s == modal_state {
                    1.0
                } else {
                    0.0
                }
            }),
        };

        Ok(Grade::Scored(Box::new(BioScore {
            subject,
            grader_id: self.grader_id.clone(),
            requirement_id: self.requirement.requirement_id.clone(),
            rule: ScoringRule::Brier,
            form,
            interval,
            resolution: distribution.resolution(),
            dispersion: distribution.dispersion(),
            reference_entropy_bits: distribution.entropy_bits(),
            bridge_loss: witness.total_bridge_loss(),
            parent_world: None,
            errors: Vec::new(),
            credit: None,
        })))
    }

    fn grade_abstention(
        &self,
        subject: String,
        reason: &str,
        reference: &ReferenceStandard,
    ) -> AbstentionRecord {
        let (warranted, modal) = match reference {
            ReferenceStandard::Distribution(d) => (
                d.entropy_bits() >= self.abstention_entropy_threshold,
                Some(d.resolution().modal_mass()),
            ),
            ReferenceStandard::Unresolved { .. } => (true, None),
            ReferenceStandard::NotEvaluable { .. } => (true, None),
        };
        AbstentionRecord {
            subject,
            reason: reason.to_string(),
            warranted,
            reference_modal_confidence: modal,
            reference_state: reference.as_str().to_string(),
        }
    }
}

/// `1 - Σ(pᵢ - tᵢ)² / 2` over the reference's state space, where `t` is supplied by `target`.
///
/// The sum runs over the reference's states, which is sound because the grader has already
/// refused any prediction placing mass outside them.
fn brier_agreement(
    predicted: &BTreeMap<String, f64>,
    reference: &ReferenceDistribution,
    target: impl Fn(&str) -> f64,
) -> f64 {
    let sum_sq: f64 = reference
        .states()
        .map(|state| {
            let p = predicted.get(state).copied().unwrap_or(0.0);
            let t = target(state);
            (p - t) * (p - t)
        })
        .sum();
    (1.0 - sum_sq / 2.0).clamp(0.0, 1.0)
}
