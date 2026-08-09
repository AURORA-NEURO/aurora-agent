//! Assay quality and missingness — blueprint 42.13.
//!
//! The lens that motivated [`crate::missingness`]. 42.13's outcome is to "distinguish biological
//! absence, technical failure, censoring, detection limits, and unknown missingness", and the
//! failure it guards against is the one a spreadsheet makes effortless: five different facts
//! about the world, rendered as five identical empty cells.
//!
//! Because [`Observation`] has no blank and [`Missingness`] has no catch-all, that collapse is
//! not a bug this lens has to detect — it is a value that cannot be built. What this lens adds is
//! the *reporting* half: a hole and a measured absence are different witness **kinds**, with
//! different sentences and different consequences for a sufficiency claim, so they stay distinct
//! all the way into a report, an export and a release gate.
//!
//! # QC has the same shape as the data
//!
//! A QC metric can itself be missing. A panel whose QC threshold was never evaluated is not a
//! panel that passed QC, and [`QcFinding::QualityNotEvaluable`] says so rather than letting the
//! absence of a breach imply quality. This is the site-check asymmetry of 42.10 appearing again
//! one layer down, which is a reason to believe it is the right shape rather than a local trick.
//!
//! # Not implemented
//!
//! **No imputation and no missingness-mechanism inference.** See [`crate::missingness`].
//! **No batch-effect detection.** 42.13's title says quality; batch structure is a cohort-level
//! confound and belongs with 42.10's site check, where the split boundary gives it a meaning.
//! **No platform-specific QC thresholds.** Thresholds arrive as input because the blueprint names
//! none and a hardcoded default would silently become a standard.

use crate::grammar::{
    AbsentRequirement, Coverage, EvidenceGap, EvidenceRequirement, Lens, LensDeclaration, LensId,
    LensOutcome, PendingRegion, RefusalReason, ScopePrecondition,
};
use crate::missingness::{Missingness, MissingnessSummary, Observation, UnattemptedReason};
use crate::nonvisual::{Cell, Witness};
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};

/// One measured or unmeasured quantity in the panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelCell {
    pub specimen: String,
    pub analyte: String,
    pub observation: Observation,
}

impl PanelCell {
    pub fn new(
        specimen: impl Into<String>,
        analyte: impl Into<String>,
        observation: Observation,
    ) -> Self {
        PanelCell {
            specimen: specimen.into(),
            analyte: analyte.into(),
            observation,
        }
    }
}

/// Which side of a QC threshold is acceptable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QcDirection {
    /// The observed value must be at least the threshold.
    AtLeast,
    /// The observed value must be at most the threshold.
    AtMost,
}

/// A quality metric for one specimen, with its threshold.
///
/// `observed` is an [`Observation`], not an `f64`, because a QC metric that was never computed is
/// the commonest way a specimen passes QC without anyone checking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QcMetric {
    pub specimen: String,
    pub metric: String,
    pub observed: Observation,
    pub threshold: f64,
    pub direction: QcDirection,
}

/// The panel under inspection.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AssayPanel {
    pub cells: Vec<PanelCell>,
    #[serde(default)]
    pub qc: Vec<QcMetric>,
    /// Specimens known to belong to this panel whose results have not arrived. Named, so a
    /// partial answer is legible as one.
    #[serde(default)]
    pub unloaded_specimens: Vec<String>,
}

impl AssayPanel {
    pub fn new(cells: Vec<PanelCell>) -> Self {
        AssayPanel {
            cells,
            qc: Vec::new(),
            unloaded_specimens: Vec::new(),
        }
    }

    pub fn with_qc(mut self, qc: Vec<QcMetric>) -> Self {
        self.qc = qc;
        self
    }

    /// Counts by missingness class. Note what this cannot give you: a single "missing" total.
    pub fn summarise(&self) -> MissingnessSummary {
        let mut summary = MissingnessSummary::new();
        for cell in &self.cells {
            summary.observe(&cell.observation);
        }
        summary
    }
}

/// What the assay QC and missingness lens found.
///
/// [`QcFinding::Hole`] and [`QcFinding::MeasuredAbsence`] are separate variants with separate
/// `kind()` strings on purpose. A consumer that groups by kind — an export, a gate, a table of
/// contents — cannot accidentally merge them, and a consumer that reads only sentences still gets
/// two different sentences.
#[derive(Debug, Clone, PartialEq)]
pub enum QcFinding {
    /// Nobody measured this quantity.
    Hole {
        specimen: String,
        analyte: String,
        missingness: Missingness,
    },
    /// Somebody measured this quantity and it was absent, or bounded. A result.
    MeasuredAbsence {
        specimen: String,
        analyte: String,
        missingness: Missingness,
    },
    /// An assay ran and failed. Neither a value nor a negative result.
    AssayFailure {
        specimen: String,
        analyte: String,
        detail: String,
    },
    /// A value exists but policy forbids reading it.
    Withheld {
        specimen: String,
        analyte: String,
        authority: String,
    },
    /// A QC metric crossed its threshold.
    QualityBreach {
        specimen: String,
        metric: String,
        observed: f64,
        threshold: f64,
    },
    /// A QC metric that was never computed, so quality is unestablished rather than acceptable.
    QualityNotEvaluable {
        specimen: String,
        metric: String,
        missingness: Missingness,
    },
}

impl QcFinding {
    /// Whether this finding records something an experiment established.
    ///
    /// False for holes and unevaluable QC. This is the predicate a sufficiency claim must consult.
    pub fn is_established(&self) -> bool {
        matches!(
            self,
            QcFinding::MeasuredAbsence { .. } | QcFinding::QualityBreach { .. }
        )
    }
}

impl Witness for QcFinding {
    fn kind(&self) -> &'static str {
        match self {
            QcFinding::Hole { .. } => "unmeasured_quantity",
            QcFinding::MeasuredAbsence { .. } => "measured_absence",
            QcFinding::AssayFailure { .. } => "assay_failure",
            QcFinding::Withheld { .. } => "withheld_quantity",
            QcFinding::QualityBreach { .. } => "quality_breach",
            QcFinding::QualityNotEvaluable { .. } => "quality_not_evaluable",
        }
    }

    fn columns(&self) -> &'static [&'static str] {
        match self {
            QcFinding::Hole { .. }
            | QcFinding::MeasuredAbsence { .. }
            | QcFinding::Withheld { .. } => &["specimen", "analyte", "state"],
            QcFinding::AssayFailure { .. } => &["specimen", "analyte", "failure"],
            QcFinding::QualityBreach { .. } => &["specimen", "metric", "observed", "threshold"],
            QcFinding::QualityNotEvaluable { .. } => &["specimen", "metric", "state"],
        }
    }

    fn cells(&self) -> Vec<Cell> {
        match self {
            QcFinding::Hole {
                specimen,
                analyte,
                missingness,
            }
            | QcFinding::MeasuredAbsence {
                specimen,
                analyte,
                missingness,
            } => vec![
                Cell::id(specimen.clone()),
                Cell::text(analyte.clone()),
                Cell::Absent {
                    missingness: missingness.clone(),
                },
            ],
            QcFinding::Withheld {
                specimen,
                analyte,
                authority,
            } => vec![
                Cell::id(specimen.clone()),
                Cell::text(analyte.clone()),
                Cell::Absent {
                    missingness: Missingness::PolicyWithheld {
                        authority: authority.clone(),
                    },
                },
            ],
            QcFinding::AssayFailure {
                specimen,
                analyte,
                detail,
            } => vec![
                Cell::id(specimen.clone()),
                Cell::text(analyte.clone()),
                Cell::text(detail.clone()),
            ],
            QcFinding::QualityBreach {
                specimen,
                metric,
                observed,
                threshold,
            } => vec![
                Cell::id(specimen.clone()),
                Cell::text(metric.clone()),
                Cell::Quantity {
                    value: *observed,
                    unit: "metric".into(),
                },
                Cell::Quantity {
                    value: *threshold,
                    unit: "metric".into(),
                },
            ],
            QcFinding::QualityNotEvaluable {
                specimen,
                metric,
                missingness,
            } => vec![
                Cell::id(specimen.clone()),
                Cell::text(metric.clone()),
                Cell::Absent {
                    missingness: missingness.clone(),
                },
            ],
        }
    }

    fn sentence(&self) -> String {
        match self {
            QcFinding::Hole {
                specimen,
                analyte,
                missingness,
            } => format!(
                "{analyte} in {specimen} has no value because {}; nothing is known about it",
                missingness.sentence()
            ),
            QcFinding::MeasuredAbsence {
                specimen,
                analyte,
                missingness,
            } => format!(
                "{analyte} in {specimen} was {}; this is a result, not a gap",
                missingness.sentence()
            ),
            QcFinding::AssayFailure {
                specimen,
                analyte,
                detail,
            } => format!("the assay for {analyte} in {specimen} failed: {detail}"),
            QcFinding::Withheld {
                specimen,
                analyte,
                authority,
            } => format!("{analyte} in {specimen} is withheld by {authority}"),
            QcFinding::QualityBreach {
                specimen,
                metric,
                observed,
                threshold,
            } => format!("{specimen} has {metric} {observed} against a threshold of {threshold}"),
            QcFinding::QualityNotEvaluable {
                specimen,
                metric,
                missingness,
            } => format!(
                "{metric} for {specimen} was never evaluated ({}), so this specimen has not \
                 passed QC — it has not been checked",
                missingness.sentence()
            ),
        }
    }
}

/// Blueprint 42.13.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssayQcMissingnessLens;

impl AssayQcMissingnessLens {
    pub const ID: &'static str = "assay_qc_missingness";

    fn cell_finding(cell: &PanelCell) -> Option<QcFinding> {
        let Observation::Absent(missingness) = &cell.observation else {
            return None;
        };
        match missingness {
            Missingness::NeverMeasured { .. } => Some(QcFinding::Hole {
                specimen: cell.specimen.clone(),
                analyte: cell.analyte.clone(),
                missingness: missingness.clone(),
            }),
            Missingness::TechnicalFailure { detail, .. } => Some(QcFinding::AssayFailure {
                specimen: cell.specimen.clone(),
                analyte: cell.analyte.clone(),
                detail: detail.clone(),
            }),
            Missingness::PolicyWithheld { authority } => Some(QcFinding::Withheld {
                specimen: cell.specimen.clone(),
                analyte: cell.analyte.clone(),
                authority: authority.clone(),
            }),
            Missingness::BiologicalAbsence { .. }
            | Missingness::Censored { .. }
            | Missingness::BelowDetectionLimit { .. } => Some(QcFinding::MeasuredAbsence {
                specimen: cell.specimen.clone(),
                analyte: cell.analyte.clone(),
                missingness: missingness.clone(),
            }),
        }
    }

    fn qc_finding(metric: &QcMetric) -> Option<QcFinding> {
        match &metric.observed {
            Observation::Absent(missingness) => Some(QcFinding::QualityNotEvaluable {
                specimen: metric.specimen.clone(),
                metric: metric.metric.clone(),
                missingness: missingness.clone(),
            }),
            Observation::Present(measured) => {
                let breached = match metric.direction {
                    QcDirection::AtLeast => measured.value() < metric.threshold,
                    QcDirection::AtMost => measured.value() > metric.threshold,
                };
                breached.then(|| QcFinding::QualityBreach {
                    specimen: metric.specimen.clone(),
                    metric: metric.metric.clone(),
                    observed: measured.value(),
                    threshold: metric.threshold,
                })
            }
        }
    }
}

impl Lens for AssayQcMissingnessLens {
    type Evidence = AssayPanel;
    type Witness = QcFinding;

    fn declaration(&self) -> LensDeclaration {
        LensDeclaration::new(
            LensId::new(Self::ID),
            "42.13",
            "for each quantity with no value, did an assay establish the absence or did nobody \
             measure it — and was quality evaluated at all?",
            vec![
                EvidenceRequirement::new(
                    "panel.cells",
                    "one observation per specimen and analyte, each absence carrying its class",
                ),
                EvidenceRequirement::new(
                    "panel.qc",
                    "quality metrics with thresholds, themselves observations",
                ),
            ],
            vec![ScopePrecondition::new(
                "specimen",
                "an assay result belongs to a specimen; unscoped, missingness has no referent",
            )],
            vec![RefusalReason::ScopePreconditionUnmet],
        )
        .expect("42.13 declaration is well formed")
    }

    fn answer(&self, _scope: &ScopeKey, panel: &AssayPanel) -> LensOutcome<QcFinding> {
        if panel.cells.is_empty() && panel.qc.is_empty() {
            let gap = EvidenceGap::new(
                Self::ID,
                vec![AbsentRequirement {
                    requirement: EvidenceRequirement::new(
                        "panel.cells",
                        "one observation per specimen and analyte",
                    ),
                    missingness: Missingness::NeverMeasured {
                        reason: UnattemptedReason::Unrecorded,
                    },
                }],
            )
            .expect("one absent requirement is not empty");
            return LensOutcome::EvidenceAbsent(gap);
        }

        let mut findings: Vec<QcFinding> =
            panel.cells.iter().filter_map(Self::cell_finding).collect();
        findings.extend(panel.qc.iter().filter_map(Self::qc_finding));

        let examined = panel.cells.len() + panel.qc.len();
        let eligible = examined + panel.unloaded_specimens.len();
        let coverage = if panel.unloaded_specimens.is_empty() {
            Coverage::complete(Self::ID, examined, eligible)
        } else {
            Coverage::partial(
                Self::ID,
                examined,
                eligible,
                panel
                    .unloaded_specimens
                    .iter()
                    .map(|s| PendingRegion::new(s.clone(), "specimen results not loaded"))
                    .collect(),
            )
        };
        match coverage {
            Ok(coverage) => LensOutcome::Answered {
                witnesses: findings,
                coverage,
            },
            Err(_) => LensOutcome::Answered {
                witnesses: findings,
                coverage: Coverage::complete(Self::ID, examined, examined)
                    .expect("examined equals itself"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::run;
    use crate::missingness::{CensorDirection, Measured};

    fn scope() -> ScopeKey {
        ScopeKey::new().exact("specimen", "SP-1")
    }

    fn hole(analyte: &str) -> PanelCell {
        PanelCell::new(
            "SP-1",
            analyte,
            Observation::Absent(Missingness::NeverMeasured {
                reason: UnattemptedReason::NotOrdered,
            }),
        )
    }

    fn measured_absent(analyte: &str) -> PanelCell {
        PanelCell::new(
            "SP-1",
            analyte,
            Observation::Absent(Missingness::BiologicalAbsence {
                assay: "IHC".into(),
            }),
        )
    }

    #[test]
    fn a_hole_and_a_measured_absence_are_different_witness_kinds() {
        let panel = AssayPanel::new(vec![hole("PDL1"), measured_absent("HER2")]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        let kinds: Vec<&str> = report.witnesses().iter().map(|r| r.kind.as_str()).collect();
        assert!(kinds.contains(&"unmeasured_quantity"));
        assert!(kinds.contains(&"measured_absence"));
    }

    #[test]
    fn a_hole_and_a_measured_absence_never_speak_the_same_sentence() {
        let panel = AssayPanel::new(vec![hole("PDL1"), measured_absent("PDL1")]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        let sentences: Vec<&str> = report
            .witnesses()
            .iter()
            .map(|r| r.sentence.as_str())
            .collect();
        assert_eq!(sentences.len(), 2);
        assert_ne!(sentences[0], sentences[1]);
        assert!(sentences.iter().any(|s| s.contains("nothing is known")));
        assert!(sentences.iter().any(|s| s.contains("not a gap")));
    }

    #[test]
    fn a_hole_row_carries_a_missingness_cell_where_a_spreadsheet_would_be_blank() {
        let panel = AssayPanel::new(vec![hole("PDL1")]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        let row = &report.witnesses()[0];
        assert!(row.contains_hole());
        assert!(row.spoken().iter().any(|s| s.contains("never measured")));
    }

    #[test]
    fn a_measured_absence_row_is_not_a_hole() {
        let panel = AssayPanel::new(vec![measured_absent("HER2")]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        assert!(!report.witnesses()[0].contains_hole());
    }

    #[test]
    fn a_censored_value_is_reported_as_a_measured_absence_not_a_hole() {
        let panel = AssayPanel::new(vec![PanelCell::new(
            "SP-1",
            "os_months",
            Observation::Absent(Missingness::Censored {
                assay: "followup".into(),
                direction: CensorDirection::Right,
                bound: 60.0,
            }),
        )]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        assert_eq!(report.witnesses()[0].kind, "measured_absence");
    }

    #[test]
    fn a_failed_assay_is_neither_a_hole_nor_a_negative_result() {
        let panel = AssayPanel::new(vec![PanelCell::new(
            "SP-1",
            "RNA",
            Observation::Absent(Missingness::TechnicalFailure {
                assay: "RNAseq".into(),
                detail: "RIN 2.1".into(),
            }),
        )]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        assert_eq!(report.witnesses()[0].kind, "assay_failure");
    }

    #[test]
    fn an_unevaluated_qc_metric_does_not_mean_the_specimen_passed_qc() {
        let panel = AssayPanel::new(vec![measured_absent("HER2")]).with_qc(vec![QcMetric {
            specimen: "SP-1".into(),
            metric: "tumour_purity".into(),
            observed: Observation::Absent(Missingness::NeverMeasured {
                reason: UnattemptedReason::NotOrdered,
            }),
            threshold: 0.2,
            direction: QcDirection::AtLeast,
        }]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "quality_not_evaluable")
            .expect("unevaluable QC reported");
        assert!(row.sentence.contains("has not been checked"));
    }

    #[test]
    fn a_qc_metric_below_its_threshold_is_a_breach_with_both_numbers() {
        let panel = AssayPanel::new(vec![measured_absent("HER2")]).with_qc(vec![QcMetric {
            specimen: "SP-1".into(),
            metric: "tumour_purity".into(),
            observed: Observation::Present(
                Measured::new(0.11, "fraction", "pathology", "tumour_purity").unwrap(),
            ),
            threshold: 0.2,
            direction: QcDirection::AtLeast,
        }]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "quality_breach")
            .expect("breach reported");
        assert!(row.sentence.contains("0.11"));
        assert!(row.sentence.contains("0.2"));
    }

    #[test]
    fn a_qc_metric_within_its_threshold_raises_nothing() {
        let panel = AssayPanel::new(vec![measured_absent("HER2")]).with_qc(vec![QcMetric {
            specimen: "SP-1".into(),
            metric: "tumour_purity".into(),
            observed: Observation::Present(
                Measured::new(0.4, "fraction", "pathology", "tumour_purity").unwrap(),
            ),
            threshold: 0.2,
            direction: QcDirection::AtLeast,
        }]);
        let report = run(&AssayQcMissingnessLens, &scope(), &panel).unwrap();
        assert_eq!(report.witnesses().len(), 1);
        assert_eq!(report.witnesses()[0].kind, "measured_absence");
    }

    #[test]
    fn a_panel_of_only_holes_does_not_support_a_sufficiency_claim() {
        let panel = AssayPanel::new(vec![hole("PDL1"), hole("HER2")]);
        assert!(!panel.summarise().supports_sufficiency_claim());
        assert_eq!(panel.summarise().holes(), 2);
        assert_eq!(panel.summarise().measured_absences(), 0);
    }

    #[test]
    fn a_panel_of_only_measured_absences_supports_a_sufficiency_claim() {
        let panel = AssayPanel::new(vec![measured_absent("PDL1"), measured_absent("HER2")]);
        assert!(panel.summarise().supports_sufficiency_claim());
    }

    #[test]
    fn an_empty_panel_reports_absent_evidence_rather_than_a_clean_panel() {
        let report = run(&AssayQcMissingnessLens, &scope(), &AssayPanel::default()).unwrap();
        assert_eq!(report.outcome().as_str(), "evidence_absent");
    }

    #[test]
    fn only_established_findings_claim_to_be_established() {
        let established = QcFinding::MeasuredAbsence {
            specimen: "SP-1".into(),
            analyte: "HER2".into(),
            missingness: Missingness::BiologicalAbsence {
                assay: "IHC".into(),
            },
        };
        let unestablished = QcFinding::Hole {
            specimen: "SP-1".into(),
            analyte: "PDL1".into(),
            missingness: Missingness::NeverMeasured {
                reason: UnattemptedReason::NotOrdered,
            },
        };
        assert!(established.is_established());
        assert!(!unestablished.is_established());
    }
}
