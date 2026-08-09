//! Methylation classification and epigenetic evidence (30.11).
//!
//! Blueprint 30.11 evaluates "preprocessing, classifier interpretation, calibration, versioning,
//! copy-number derivation, and cross-assay integration". This module implements the interpretation
//! half: what a classifier is allowed to emit, and what a reader is allowed to do with it.
//!
//! # Unclassifiable is a result
//!
//! [`MethylationOutcome`] has two variants and the second is not a failure.
//! [`MethylationOutcome::Unclassifiable`] carries a reason and, optionally, the class that scored
//! highest — and there is no function anywhere that turns that diagnostic into a call.
//! [`MethylationOutcome::class`] returns `None`, [`MethylationOutcome::require_class`] returns
//! [`MethylationRefusal::Unclassifiable`], and the nearest class lives behind
//! [`NearestClass`], a type with no conversion into [`MethylationClass`].
//!
//! It matters that this is a *result* rather than a missing value: 30.11 names "discarding
//! borderline cases from evaluation" as a characteristic failure, so
//! [`EvaluationCohort::denominator`] counts unclassifiable samples. A classifier that abstains on
//! a third of a cohort has told you something, and a metric computed over the other two thirds has
//! hidden it.
//!
//! # Calibrated and raw scores are different types
//!
//! [`RawScore`] and [`CalibratedScore`] do not convert implicitly. A raw score becomes calibrated
//! only through [`RawScore::calibrate`], which attaches the [`Calibration`] it went through, and
//! [`compare_raw_across_versions`] refuses outright when the versions differ — 30.11 names
//! "comparing uncalibrated scores across versions" as a failure and the type system is where that
//! is cheapest to stop. Neither type has an `is_certain`, because "treating maximum score as
//! certainty" is the first failure on the list.
//!
//! # A version change is not a correction
//!
//! The module's worked microbenchmark is a sample that "moves from one low-confidence class to
//! another after a reference update while its copy-number profile is stable", where the system
//! "must report version-conditioned evidence rather than call one historical result wrong".
//! [`reconcile_versions`] has no variant meaning "the earlier call was wrong"; a disagreement
//! across versions is [`VersionDivergence::VersionConditioned`], and the older result stays a
//! true statement about the older classifier.
//!
//! # Vocabulary and thresholds
//!
//! [`MethylationClass`] is an **opaque string newtype**, not an enumeration. 30.11 names no
//! methylation class, no subclass and no reference cohort, and writing a list of class names here
//! would be this crate asserting a taxonomy under the blueprint's authority. For the same reason
//! [`ClassifierVersion::reporting_threshold`] is an `Option` with no default and
//! [`classify`] refuses when it is absent: the score above which a classifier reports a class is
//! a property of that classifier's validation, and a number invented here would read as domain
//! knowledge.
//!
//! # Not implemented
//!
//! No array reading, no normalisation, no batch correction, no copy-number derivation from
//! intensities, no trained classifier and no reference cohort. This module never computes a score;
//! it constrains what may be said about one that a classifier produced.

use crate::error::{FractionError, MethylationRefusal};
use bioprism_onco::Observed;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A score in `[0, 1]`, in parts per ten thousand.
///
/// Integer-backed for the same reason [`crate::clonal::CellularFraction`] is: comparisons against
/// a reporting threshold must be decidable, not a question about the last bit of a float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScoreValue {
    parts_per_ten_thousand: u16,
}

impl ScoreValue {
    pub const fn from_parts_per_ten_thousand(parts: u16) -> Result<Self, FractionError> {
        if parts > 10_000 {
            return Err(FractionError::AboveWhole {
                parts: parts as u32,
            });
        }
        Ok(ScoreValue {
            parts_per_ten_thousand: parts,
        })
    }

    pub fn from_ratio(value: f64) -> Result<Self, FractionError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(FractionError::NotAUnitRatio {
                value: format!("{value}"),
            });
        }
        ScoreValue::from_parts_per_ten_thousand((value * 10_000.0).round() as u16)
    }

    pub const fn parts_per_ten_thousand(self) -> u16 {
        self.parts_per_ten_thousand
    }

    fn describe(self) -> String {
        format!("{}/10000", self.parts_per_ten_thousand)
    }
}

/// A classifier output before calibration.
///
/// Has no ordering relation to any threshold and no comparison across classifier versions. It is
/// the number the model emitted, and on its own that is all it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RawScore(pub ScoreValue);

impl RawScore {
    pub fn calibrate(self, calibration: &Calibration) -> CalibratedScore {
        CalibratedScore {
            value: self.0,
            calibration: calibration.clone(),
        }
    }
}

/// How a raw score was mapped onto a calibrated one.
///
/// `method` is an opaque string. 30.11 requires calibration and names no method, so an enum here
/// would be an invented taxonomy of calibration procedures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calibration {
    pub method: String,
    pub version: String,
}

impl Calibration {
    pub fn new(method: impl Into<String>, version: impl Into<String>) -> Self {
        Calibration {
            method: method.into(),
            version: version.into(),
        }
    }
}

/// A score that carries the calibration it went through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibratedScore {
    pub value: ScoreValue,
    pub calibration: Calibration,
}

impl CalibratedScore {
    pub fn at_or_above(&self, threshold: ScoreValue) -> bool {
        self.value >= threshold
    }
}

/// A methylation class label, as emitted by some classifier version.
///
/// Opaque by design; see the module header. Two labels from different classifier versions may be
/// spelled identically and mean different things, which is why [`VersionedResult`] always carries
/// its [`ClassifierVersion`] alongside.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MethylationClass(String);

impl MethylationClass {
    pub fn new(label: impl Into<String>) -> Self {
        MethylationClass(label.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A classifier and the reference it was trained against (30.11, "classifier and reference
/// version").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassifierVersion {
    pub name: String,
    pub version: String,
    pub reference_version: String,
    /// The calibrated score at or above which this classifier reports a class.
    ///
    /// No default and no fallback. Supplied by whoever validated the classifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reporting_threshold: Option<ScoreValue>,
}

impl ClassifierVersion {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        reference_version: impl Into<String>,
    ) -> Self {
        ClassifierVersion {
            name: name.into(),
            version: version.into(),
            reference_version: reference_version.into(),
            reporting_threshold: None,
        }
    }

    pub fn reporting_at(mut self, threshold: ScoreValue) -> Self {
        self.reporting_threshold = Some(threshold);
        self
    }

    pub fn identity(&self) -> String {
        format!("{} {} / ref {}", self.name, self.version, self.reference_version)
    }
}

/// Quality-control outcome for the array or library (30.11, "quality-control outputs").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "qc", rename_all = "snake_case")]
pub enum QcOutcome {
    Passed,
    Failed { detail: String },
}

/// The material a classification was run on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleContext {
    pub qc: QcOutcome,
    /// Tumour content, using `bioprism_onco`'s absence states so that "not measured" and
    /// "measured and low" stay distinct.
    pub tumour_content: Observed<crate::clonal::CellularFraction>,
}

impl SampleContext {
    pub fn new(qc: QcOutcome, tumour_content: Observed<crate::clonal::CellularFraction>) -> Self {
        SampleContext { qc, tumour_content }
    }
}

/// Why no class was reported.
///
/// [`UnclassifiableReason::NoClassAboveThreshold`] is the ordinary case and is not an error: the
/// classifier ran, produced scores, and none of them cleared the bar its own validation set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum UnclassifiableReason {
    QualityControlFailure { detail: String },
    NoClassAboveThreshold { best: ScoreValue, threshold: ScoreValue },
    NoScoresSubmitted,
}

/// The class that scored highest when nothing cleared the threshold.
///
/// Exists so that a reader can see how close the call was, and has no method returning a
/// [`MethylationClass`]. Reaching the label requires reading the field of a type whose name says
/// what it is, which is the point: 30.11 forbids silently assigning a borderline sample to its
/// nearest class, and the way that happens in code is an innocuous-looking accessor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NearestClass {
    pub label_only: MethylationClass,
    pub score: CalibratedScore,
}

/// What a methylation classifier produced for one sample.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum MethylationOutcome {
    Classified {
        class: MethylationClass,
        score: CalibratedScore,
    },
    Unclassifiable {
        reason: UnclassifiableReason,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nearest: Option<NearestClass>,
    },
}

impl MethylationOutcome {
    pub fn class(&self) -> Option<&MethylationClass> {
        match self {
            MethylationOutcome::Classified { class, .. } => Some(class),
            MethylationOutcome::Unclassifiable { .. } => None,
        }
    }

    pub fn require_class(&self) -> Result<&MethylationClass, MethylationRefusal> {
        match self {
            MethylationOutcome::Classified { class, .. } => Ok(class),
            MethylationOutcome::Unclassifiable { reason, .. } => {
                Err(MethylationRefusal::Unclassifiable {
                    reason: describe_reason(reason),
                })
            }
        }
    }

    pub fn is_classified(&self) -> bool {
        matches!(self, MethylationOutcome::Classified { .. })
    }
}

fn describe_reason(reason: &UnclassifiableReason) -> String {
    match reason {
        UnclassifiableReason::QualityControlFailure { detail } => {
            format!("quality control failed: {detail}")
        }
        UnclassifiableReason::NoClassAboveThreshold { best, threshold } => format!(
            "best calibrated score {} is below the reporting threshold {}",
            best.describe(),
            threshold.describe()
        ),
        UnclassifiableReason::NoScoresSubmitted => "no class scores were submitted".to_string(),
    }
}

/// A classification with everything a reader needs to condition on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassificationReport {
    pub classifier: ClassifierVersion,
    pub outcome: MethylationOutcome,
    /// Things that did not prevent a call but that a reader must know.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caveats: Vec<String>,
}

/// Turns calibrated per-class scores into an outcome.
///
/// Refuses when the classifier declares no reporting threshold, because without one the only
/// available rule is "report the maximum", which is exactly the failure 30.11 lists first.
///
/// Undeclared tumour content produces a caveat rather than a refusal. 30.11 names "ignoring low
/// tumor content" as a failure but specifies no minimum, and a minimum invented here would be
/// presented as validation evidence that does not exist.
pub fn classify(
    classifier: &ClassifierVersion,
    scores: &BTreeMap<MethylationClass, CalibratedScore>,
    context: &SampleContext,
) -> Result<ClassificationReport, MethylationRefusal> {
    let threshold =
        classifier
            .reporting_threshold
            .ok_or_else(|| MethylationRefusal::UndeclaredThreshold {
                classifier: classifier.identity(),
            })?;

    let mut caveats = Vec::new();
    match &context.tumour_content {
        Observed::Value(_) => {}
        Observed::Unobserved(status) => caveats.push(format!(
            "tumour content is {}; class evidence is not conditioned on material context",
            status.describe()
        )),
    }

    if let QcOutcome::Failed { detail } = &context.qc {
        return Ok(ClassificationReport {
            classifier: classifier.clone(),
            outcome: MethylationOutcome::Unclassifiable {
                reason: UnclassifiableReason::QualityControlFailure {
                    detail: detail.clone(),
                },
                nearest: None,
            },
            caveats,
        });
    }

    let best = scores
        .iter()
        .max_by(|left, right| left.1.value.cmp(&right.1.value));
    let outcome = match best {
        None => MethylationOutcome::Unclassifiable {
            reason: UnclassifiableReason::NoScoresSubmitted,
            nearest: None,
        },
        Some((class, score)) if score.at_or_above(threshold) => MethylationOutcome::Classified {
            class: class.clone(),
            score: score.clone(),
        },
        Some((class, score)) => MethylationOutcome::Unclassifiable {
            reason: UnclassifiableReason::NoClassAboveThreshold {
                best: score.value,
                threshold,
            },
            nearest: Some(NearestClass {
                label_only: class.clone(),
                score: score.clone(),
            }),
        },
    };

    Ok(ClassificationReport {
        classifier: classifier.clone(),
        outcome,
        caveats,
    })
}

/// Comparing two raw scores, which is only defined within one classifier version.
pub fn compare_raw_across_versions(
    left: (&ClassifierVersion, RawScore),
    right: (&ClassifierVersion, RawScore),
) -> Result<std::cmp::Ordering, MethylationRefusal> {
    if left.0.identity() != right.0.identity() {
        return Err(MethylationRefusal::UncalibratedCrossVersion {
            left: left.0.identity(),
            right: right.0.identity(),
        });
    }
    Ok(left.1 .0.cmp(&right.1 .0))
}

/// A classification pinned to the version that produced it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedResult {
    pub classifier: ClassifierVersion,
    pub outcome: MethylationOutcome,
}

/// How two classifier versions relate on one sample.
///
/// There is no `EarlierResultWrong`. A result is a true statement about the classifier that
/// produced it, and a reference update changes what the question means rather than revealing that
/// the old answer was a mistake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "divergence", rename_all = "snake_case")]
pub enum VersionDivergence {
    Agree { class: MethylationClass },
    BothUnclassifiable,
    /// One version reports a class the other does not, or a different class.
    VersionConditioned {
        under_left: Option<MethylationClass>,
        under_right: Option<MethylationClass>,
    },
}

/// A cross-version comparison and the corroborating evidence that did not move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionComparison {
    pub left: ClassifierVersion,
    pub right: ClassifierVersion,
    pub divergence: VersionDivergence,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stable_evidence: Vec<String>,
}

/// Compares two versioned results without adjudicating between them.
pub fn reconcile_versions(left: &VersionedResult, right: &VersionedResult) -> VersionComparison {
    let divergence = match (left.outcome.class(), right.outcome.class()) {
        (Some(a), Some(b)) if a == b => VersionDivergence::Agree { class: a.clone() },
        (None, None) => VersionDivergence::BothUnclassifiable,
        (a, b) => VersionDivergence::VersionConditioned {
            under_left: a.cloned(),
            under_right: b.cloned(),
        },
    };
    VersionComparison {
        left: left.classifier.clone(),
        right: right.classifier.clone(),
        divergence,
        stable_evidence: Vec::new(),
    }
}

/// Where a copy-number profile came from (30.11, "copy-number profile derivation").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub enum CopyNumberProvenance {
    /// Derived from the intensities of the same array the class call came from.
    DerivedFromClassifiedArray,
    IndependentAssay { assay: String },
}

/// A corroboration of a class call by copy-number evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Corroboration {
    pub class: MethylationClass,
    pub assay: String,
}

/// Whether a copy-number profile independently corroborates a class call.
///
/// A profile derived from the same array is not independent evidence about the call that array
/// produced. 30.11's ladder item is "use copy-number evidence without circularity"; this is the
/// circularity.
pub fn corroborate(
    outcome: &MethylationOutcome,
    provenance: &CopyNumberProvenance,
) -> Result<Corroboration, MethylationRefusal> {
    let class = outcome.require_class()?;
    match provenance {
        CopyNumberProvenance::DerivedFromClassifiedArray => {
            Err(MethylationRefusal::CircularCopyNumber)
        }
        CopyNumberProvenance::IndependentAssay { assay } => Ok(Corroboration {
            class: class.clone(),
            assay: assay.clone(),
        }),
    }
}

/// The role a variable plays in an analysis (30.11, "using classifier label as both feature and
/// target").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceUse {
    Feature,
    Target,
}

impl EvidenceUse {
    pub const fn as_str(self) -> &'static str {
        match self {
            EvidenceUse::Feature => "a feature",
            EvidenceUse::Target => "the target",
        }
    }
}

/// Which role each variable has been given in one analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleLedger {
    roles: BTreeMap<String, EvidenceUse>,
}

impl RoleLedger {
    pub fn new() -> Self {
        RoleLedger::default()
    }

    /// Records a role, refusing a second, different role for the same variable.
    pub fn use_as(
        &mut self,
        variable: impl Into<String>,
        use_: EvidenceUse,
    ) -> Result<(), MethylationRefusal> {
        let variable = variable.into();
        if let Some(existing) = self.roles.get(&variable) {
            if *existing != use_ {
                return Err(MethylationRefusal::CircularLabelUse {
                    existing_use: existing.as_str().to_string(),
                    requested_use: use_.as_str().to_string(),
                });
            }
        }
        self.roles.insert(variable, use_);
        Ok(())
    }

    pub fn role_of(&self, variable: &str) -> Option<EvidenceUse> {
        self.roles.get(variable).copied()
    }
}

/// A cohort of classification results, with the abstentions kept in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationCohort {
    pub classified: Vec<VersionedResult>,
    pub unclassifiable: Vec<VersionedResult>,
}

impl EvaluationCohort {
    pub fn from_results(results: Vec<VersionedResult>) -> Self {
        let mut cohort = EvaluationCohort::default();
        for result in results {
            if result.outcome.is_classified() {
                cohort.classified.push(result);
            } else {
                cohort.unclassifiable.push(result);
            }
        }
        cohort
    }

    /// Every sample the classifier was asked about.
    ///
    /// Abstentions are in the denominator. A classifier that answers rarely and well is a
    /// different object from one that answers always and well, and 30.11 forbids the report that
    /// cannot tell them apart.
    pub fn denominator(&self) -> usize {
        self.classified.len() + self.unclassifiable.len()
    }

    pub fn abstention_count(&self) -> usize {
        self.unclassifiable.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clonal::CellularFraction;
    use bioprism_onco::ObservationStatus;

    fn score(parts: u16) -> ScoreValue {
        ScoreValue::from_parts_per_ten_thousand(parts).expect("within the unit interval")
    }

    fn calibration() -> Calibration {
        Calibration::new(
            "as documented by the classifier's validation report",
            "cal-1",
        )
    }

    fn calibrated(parts: u16) -> CalibratedScore {
        RawScore(score(parts)).calibrate(&calibration())
    }

    fn classifier(version: &str, threshold: u16) -> ClassifierVersion {
        ClassifierVersion::new("illustrative classifier", version, "ref-1")
            .reporting_at(score(threshold))
    }

    fn passing_context() -> SampleContext {
        SampleContext::new(
            QcOutcome::Passed,
            Observed::Value(
                CellularFraction::from_parts_per_ten_thousand(7_000).expect("within the whole"),
            ),
        )
    }

    fn scores(pairs: &[(&str, u16)]) -> BTreeMap<MethylationClass, CalibratedScore> {
        pairs
            .iter()
            .map(|(label, parts)| (MethylationClass::new(*label), calibrated(*parts)))
            .collect()
    }

    #[test]
    fn a_classifier_without_a_declared_threshold_cannot_emit_a_class() {
        let bare = ClassifierVersion::new("illustrative classifier", "v1", "ref-1");
        let refusal = classify(&bare, &scores(&[("class-a", 9_900)]), &passing_context())
            .unwrap_err();
        assert!(matches!(
            refusal,
            MethylationRefusal::UndeclaredThreshold { .. }
        ));
    }

    #[test]
    fn an_unclassifiable_sample_is_a_result_not_a_missing_value() {
        let report = classify(
            &classifier("v1", 9_000),
            &scores(&[("class-a", 4_000), ("class-b", 3_500)]),
            &passing_context(),
        )
        .expect("the classifier declares a threshold");
        assert!(!report.outcome.is_classified());
        assert!(matches!(
            report.outcome,
            MethylationOutcome::Unclassifiable {
                reason: UnclassifiableReason::NoClassAboveThreshold { .. },
                ..
            }
        ));
    }

    #[test]
    fn an_unclassifiable_sample_is_never_silently_assigned_to_the_nearest_class() {
        let report = classify(
            &classifier("v1", 9_000),
            &scores(&[("class-a", 8_999), ("class-b", 10)]),
            &passing_context(),
        )
        .expect("the classifier declares a threshold");
        assert!(report.outcome.class().is_none());
        assert!(report.outcome.require_class().is_err());
        let MethylationOutcome::Unclassifiable { nearest, .. } = &report.outcome else {
            panic!("expected an abstention");
        };
        let nearest = nearest.as_ref().expect("the near miss is visible");
        assert_eq!(nearest.label_only.as_str(), "class-a");
    }

    #[test]
    fn the_highest_score_is_a_class_only_once_it_clears_the_declared_threshold() {
        let below = classify(
            &classifier("v1", 9_000),
            &scores(&[("class-a", 8_999)]),
            &passing_context(),
        )
        .unwrap();
        let at = classify(
            &classifier("v1", 9_000),
            &scores(&[("class-a", 9_000)]),
            &passing_context(),
        )
        .unwrap();
        assert!(below.outcome.class().is_none());
        assert_eq!(at.outcome.class().unwrap().as_str(), "class-a");
    }

    #[test]
    fn a_failed_quality_control_abstains_rather_than_classifying() {
        let context = SampleContext::new(
            QcOutcome::Failed {
                detail: "control probe intensities out of range".to_string(),
            },
            Observed::Value(CellularFraction::WHOLE),
        );
        let report = classify(
            &classifier("v1", 1_000),
            &scores(&[("class-a", 9_900)]),
            &context,
        )
        .unwrap();
        assert!(matches!(
            report.outcome,
            MethylationOutcome::Unclassifiable {
                reason: UnclassifiableReason::QualityControlFailure { .. },
                ..
            }
        ));
    }

    #[test]
    fn undeclared_tumour_content_produces_a_caveat_rather_than_an_invented_minimum() {
        let context = SampleContext::new(
            QcOutcome::Passed,
            Observed::Unobserved(ObservationStatus::NotCollected),
        );
        let report = classify(
            &classifier("v1", 5_000),
            &scores(&[("class-a", 9_000)]),
            &context,
        )
        .unwrap();
        assert!(report.outcome.is_classified());
        assert_eq!(report.caveats.len(), 1);
        assert!(report.caveats[0].contains("never collected"));
    }

    #[test]
    fn raw_scores_from_two_classifier_versions_are_not_comparable() {
        let refusal = compare_raw_across_versions(
            (&classifier("v1", 5_000), RawScore(score(8_000))),
            (&classifier("v2", 5_000), RawScore(score(7_000))),
        )
        .unwrap_err();
        assert!(matches!(
            refusal,
            MethylationRefusal::UncalibratedCrossVersion { .. }
        ));
        assert!(compare_raw_across_versions(
            (&classifier("v1", 5_000), RawScore(score(8_000))),
            (&classifier("v1", 5_000), RawScore(score(7_000))),
        )
        .is_ok());
    }

    #[test]
    fn a_class_change_across_versions_is_version_conditioned_not_a_correction() {
        let old = VersionedResult {
            classifier: classifier("v1", 5_000),
            outcome: MethylationOutcome::Classified {
                class: MethylationClass::new("class-a"),
                score: calibrated(5_100),
            },
        };
        let new = VersionedResult {
            classifier: classifier("v2", 5_000),
            outcome: MethylationOutcome::Classified {
                class: MethylationClass::new("class-b"),
                score: calibrated(5_200),
            },
        };
        let comparison = reconcile_versions(&old, &new);
        assert!(matches!(
            comparison.divergence,
            VersionDivergence::VersionConditioned { .. }
        ));
        let encoded = serde_json::to_string(&comparison).expect("comparison serialises");
        assert!(!encoded.contains("wrong"));
        assert!(encoded.contains("version_conditioned"));
    }

    #[test]
    fn two_versions_agreeing_on_a_class_is_its_own_outcome() {
        let result = |version: &str| VersionedResult {
            classifier: classifier(version, 5_000),
            outcome: MethylationOutcome::Classified {
                class: MethylationClass::new("class-a"),
                score: calibrated(9_000),
            },
        };
        assert!(matches!(
            reconcile_versions(&result("v1"), &result("v2")).divergence,
            VersionDivergence::Agree { .. }
        ));
    }

    #[test]
    fn copy_number_derived_from_the_classified_array_is_not_independent_corroboration() {
        let outcome = MethylationOutcome::Classified {
            class: MethylationClass::new("class-a"),
            score: calibrated(9_500),
        };
        assert_eq!(
            corroborate(&outcome, &CopyNumberProvenance::DerivedFromClassifiedArray).unwrap_err(),
            MethylationRefusal::CircularCopyNumber
        );
        assert!(corroborate(
            &outcome,
            &CopyNumberProvenance::IndependentAssay {
                assay: "an orthogonal copy-number assay".to_string()
            }
        )
        .is_ok());
    }

    #[test]
    fn a_class_label_cannot_be_both_a_feature_and_the_target() {
        let mut ledger = RoleLedger::new();
        ledger
            .use_as("methylation class", EvidenceUse::Target)
            .expect("first declaration is free");
        let refusal = ledger
            .use_as("methylation class", EvidenceUse::Feature)
            .unwrap_err();
        assert!(matches!(
            refusal,
            MethylationRefusal::CircularLabelUse { .. }
        ));
        assert_eq!(
            ledger.role_of("methylation class"),
            Some(EvidenceUse::Target)
        );
    }

    #[test]
    fn unclassifiable_samples_stay_in_the_denominator() {
        let classified = VersionedResult {
            classifier: classifier("v1", 5_000),
            outcome: MethylationOutcome::Classified {
                class: MethylationClass::new("class-a"),
                score: calibrated(9_000),
            },
        };
        let abstained = VersionedResult {
            classifier: classifier("v1", 5_000),
            outcome: MethylationOutcome::Unclassifiable {
                reason: UnclassifiableReason::NoClassAboveThreshold {
                    best: score(100),
                    threshold: score(5_000),
                },
                nearest: None,
            },
        };
        let cohort = EvaluationCohort::from_results(vec![classified, abstained.clone(), abstained]);
        assert_eq!(cohort.denominator(), 3);
        assert_eq!(cohort.abstention_count(), 2);
        assert_eq!(cohort.classified.len(), 1);
    }

    #[test]
    fn an_empty_score_set_abstains_rather_than_panicking() {
        let report = classify(&classifier("v1", 5_000), &BTreeMap::new(), &passing_context())
            .expect("the classifier declares a threshold");
        assert!(matches!(
            report.outcome,
            MethylationOutcome::Unclassifiable {
                reason: UnclassifiableReason::NoScoresSubmitted,
                ..
            }
        ));
    }
}
