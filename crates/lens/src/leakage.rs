//! Cohort split and leakage — blueprint 42.10.
//!
//! 42.10 asks to "visualize subject, site, scanner, time, specimen, preprocessing, and label
//! dependencies crossing split boundaries". The visualization is not the content; the dependency
//! is, and a dependency crossing a split boundary is a *witness* — "alias ALT-77 resolves to
//! subjects S001 and S003, which land in different splits" — exactly the object 43.41 specifies
//! and `bioprism-section` already types. This lens therefore emits
//! [`bioprism_section::LeakageWitness`] rather than inventing a parallel vocabulary, so a leakage
//! finding surfaced in the hub and one produced by the split-integrity oracle are the same value.
//!
//! # The asymmetry that makes this lens honest
//!
//! Four checks run here — identity, site, temporal, preprocessing — and each depends on a field
//! that a cohort record may simply not carry. If a subject's site is unrecorded, the site check
//! did not fail; it did not run. A lens that reports "no leakage found" in that situation has
//! converted a hole into a clean bill of health, which is the cohort-level version of the
//! unmeasured-is-not-zero rule.
//!
//! [`LeakageFinding::CheckNotRunnable`] is therefore a finding in its own right, and
//! [`leakage_status`] returns [`OracleStatus::Underdetermined`] whenever one is present. A run
//! with no leaks and one unrunnable check is **not** valid. That is the single most important
//! line in this module.
//!
//! # Not implemented
//!
//! No split *repair* and no split *generation*. This lens detects; constructing a leakage-free
//! partition is a different problem with different failure modes, and 42.10 does not ask for it.
//! No scanner-level check either: scanner confounding is structurally the site check with a
//! different field name, and duplicating it would inflate the module count without adding a
//! distinguishable failure.

use crate::error::LensError;
use crate::grammar::{
    Coverage, EvidenceRequirement, Lens, LensDeclaration, LensId, LensOutcome, RefusalReason,
    ScopePrecondition,
};
use crate::missingness::{Missingness, Recorded};
use crate::nonvisual::{Cell, Witness};
use bioprism_scope::{ScopeKey, Timestamp};
use bioprism_section::{LeakageWitness, OracleStatus};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One subject's split membership and the attributes the four checks read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubjectRecord {
    pub subject: String,
    pub split: String,
    /// Every identifier this subject is known by. A shared alias across splits is the classic
    /// identity leak.
    pub aliases: Vec<String>,
    pub site: Recorded<String>,
    /// When the evidence backing this subject's label came into existence, RFC 3339.
    pub label_source_time: Recorded<String>,
}

/// A preprocessing step and the splits its parameters were fit over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreprocessingStep {
    pub name: String,
    /// The splits whose data entered the fit. More than one is a leak.
    pub fit_over: Vec<String>,
}

/// Everything the leakage lens reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohortSplit {
    pub subjects: Vec<SubjectRecord>,
    pub preprocessing: Vec<PreprocessingStep>,
    /// The moment the training decision was made. A label sourced after it is a temporal leak.
    pub decision_time: Recorded<String>,
}

impl CohortSplit {
    pub fn new(subjects: Vec<SubjectRecord>) -> Self {
        CohortSplit {
            subjects,
            preprocessing: Vec::new(),
            decision_time: Recorded::unrecorded(),
        }
    }

    pub fn with_preprocessing(mut self, steps: Vec<PreprocessingStep>) -> Self {
        self.preprocessing = steps;
        self
    }

    pub fn with_decision_time(mut self, time: impl Into<String>) -> Self {
        self.decision_time = Recorded::known(time.into());
        self
    }
}

/// What the leakage lens found, or could not look for.
#[derive(Debug, Clone, PartialEq)]
pub enum LeakageFinding {
    /// A dependency crossing a split boundary, in the shared vocabulary of 43.41.
    Leak(LeakageWitness),
    /// A check whose input was absent. Not a pass, not a failure — a gap, named.
    CheckNotRunnable {
        check: &'static str,
        subjects: Vec<String>,
        missingness: Missingness,
    },
}

impl LeakageFinding {
    /// Whether this finding is a leak rather than a gap.
    pub fn is_leak(&self) -> bool {
        matches!(self, LeakageFinding::Leak(_))
    }
}

impl Witness for LeakageFinding {
    fn kind(&self) -> &'static str {
        match self {
            LeakageFinding::Leak(witness) => witness.kind(),
            LeakageFinding::CheckNotRunnable { .. } => "check_not_runnable",
        }
    }

    fn columns(&self) -> &'static [&'static str] {
        match self {
            LeakageFinding::Leak(_) => &["check", "crossing", "splits"],
            LeakageFinding::CheckNotRunnable { .. } => &["check", "subjects", "why"],
        }
    }

    fn cells(&self) -> Vec<Cell> {
        match self {
            LeakageFinding::Leak(witness) => match witness {
                LeakageWitness::IdentityLeakage {
                    alias,
                    subjects,
                    splits,
                } => vec![
                    Cell::text("identity"),
                    Cell::text(format!("alias {alias} -> subjects {}", subjects.join(", "))),
                    Cell::text(splits.join(", ")),
                ],
                LeakageWitness::SiteLeakage { site_by_split } => {
                    let pairs: Vec<String> = site_by_split
                        .iter()
                        .map(|(split, sites)| format!("{split}={}", sites.join("/")))
                        .collect();
                    vec![
                        Cell::text("site"),
                        Cell::text("each split draws from one site and the sites differ"),
                        Cell::text(pairs.join("; ")),
                    ]
                }
                LeakageWitness::TemporalLeakage {
                    decision_time,
                    future_label_sources,
                } => {
                    let pairs: Vec<String> = future_label_sources
                        .iter()
                        .map(|(subject, at)| format!("{subject}@{at}"))
                        .collect();
                    vec![
                        Cell::text("temporal"),
                        Cell::text(format!("label sources after {decision_time}")),
                        Cell::text(pairs.join(", ")),
                    ]
                }
                LeakageWitness::PreprocessingLeakage { detail } => vec![
                    Cell::text("preprocessing"),
                    Cell::text(detail.clone()),
                    Cell::text("fit spans splits"),
                ],
                LeakageWitness::DomainCheck {
                    check,
                    observed,
                    detail,
                } => vec![
                    Cell::text(check.clone()),
                    Cell::text(detail.clone()),
                    Cell::text(
                        observed
                            .iter()
                            .map(|(variable, value)| format!("{variable}={value}"))
                            .collect::<Vec<_>>()
                            .join("; "),
                    ),
                ],
            },
            LeakageFinding::CheckNotRunnable {
                check,
                subjects,
                missingness,
            } => vec![
                Cell::text(*check),
                Cell::count(subjects.len()),
                Cell::Absent {
                    missingness: missingness.clone(),
                },
            ],
        }
    }

    fn sentence(&self) -> String {
        match self {
            LeakageFinding::Leak(witness) => match witness {
                LeakageWitness::IdentityLeakage {
                    alias,
                    subjects,
                    splits,
                } => format!(
                    "alias {alias} resolves to subjects {} which land in splits {}",
                    subjects.join(", "),
                    splits.join(", ")
                ),
                LeakageWitness::SiteLeakage { site_by_split } => format!(
                    "site is confounded with the split: {}",
                    site_by_split
                        .iter()
                        .map(|(split, sites)| format!(
                            "{split} draws only from {}",
                            sites.join("/")
                        ))
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
                LeakageWitness::TemporalLeakage {
                    decision_time,
                    future_label_sources,
                } => format!(
                    "{} label(s) were sourced after the decision time {decision_time}",
                    future_label_sources.len()
                ),
                LeakageWitness::PreprocessingLeakage { detail } => {
                    format!("preprocessing leaked across splits: {detail}")
                }
                LeakageWitness::DomainCheck { check, detail, .. } => {
                    format!("domain check {check} fired: {detail}")
                }
            },
            LeakageFinding::CheckNotRunnable {
                check,
                subjects,
                missingness,
            } => format!(
                "the {check} check did not run for {} subject(s): {}",
                subjects.len(),
                missingness.sentence()
            ),
        }
    }
}

/// The verdict over a set of findings.
///
/// [`OracleStatus::Valid`] requires every check to have run *and* found nothing. One unrunnable
/// check yields [`OracleStatus::Underdetermined`], because "we did not look" is not evidence of
/// absence and 43.28 requires abstention to be representable rather than rounded to a pass.
pub fn leakage_status(findings: &[LeakageFinding]) -> OracleStatus {
    if findings.iter().any(LeakageFinding::is_leak) {
        OracleStatus::Invalid
    } else if findings.is_empty() {
        OracleStatus::Valid
    } else {
        OracleStatus::Underdetermined
    }
}

/// Blueprint 42.10.
#[derive(Debug, Clone, Copy, Default)]
pub struct CohortLeakageLens;

impl CohortLeakageLens {
    pub const ID: &'static str = "cohort_leakage";

    fn identity_leaks(&self, cohort: &CohortSplit) -> Vec<LeakageFinding> {
        let mut by_alias: BTreeMap<&str, BTreeMap<&str, &str>> = BTreeMap::new();
        for record in &cohort.subjects {
            for alias in &record.aliases {
                by_alias
                    .entry(alias.as_str())
                    .or_default()
                    .insert(record.subject.as_str(), record.split.as_str());
            }
        }
        by_alias
            .into_iter()
            .filter_map(|(alias, members)| {
                let splits: Vec<String> = members
                    .values()
                    .map(|split| (*split).to_string())
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                if splits.len() < 2 {
                    return None;
                }
                Some(LeakageFinding::Leak(LeakageWitness::IdentityLeakage {
                    alias: alias.to_string(),
                    subjects: members.keys().map(|s| (*s).to_string()).collect(),
                    splits,
                }))
            })
            .collect()
    }

    fn site_findings(&self, cohort: &CohortSplit) -> Vec<LeakageFinding> {
        let unrecorded: Vec<String> = cohort
            .subjects
            .iter()
            .filter(|r| !r.site.is_known())
            .map(|r| r.subject.clone())
            .collect();
        if !unrecorded.is_empty() {
            let missingness = cohort
                .subjects
                .iter()
                .find_map(|r| r.site.missingness().cloned())
                .unwrap_or(Missingness::NeverMeasured {
                    reason: crate::missingness::UnattemptedReason::Unrecorded,
                });
            return vec![LeakageFinding::CheckNotRunnable {
                check: "site",
                subjects: unrecorded,
                missingness,
            }];
        }

        let mut sites_by_split: BTreeMap<String, std::collections::BTreeSet<String>> =
            BTreeMap::new();
        for record in &cohort.subjects {
            if let Some(site) = record.site.value() {
                sites_by_split
                    .entry(record.split.clone())
                    .or_default()
                    .insert(site.clone());
            }
        }
        let single_site_splits = sites_by_split.values().all(|sites| sites.len() == 1);
        let all_sites: std::collections::BTreeSet<&String> =
            sites_by_split.values().flatten().collect();
        if sites_by_split.len() > 1 && single_site_splits && all_sites.len() == sites_by_split.len()
        {
            let site_by_split = sites_by_split
                .into_iter()
                .map(|(split, sites)| (split, sites.into_iter().collect::<Vec<_>>()))
                .collect();
            vec![LeakageFinding::Leak(LeakageWitness::SiteLeakage {
                site_by_split,
            })]
        } else {
            Vec::new()
        }
    }

    fn temporal_findings(&self, cohort: &CohortSplit) -> Vec<LeakageFinding> {
        let Some(raw_decision) = cohort.decision_time.value() else {
            return vec![LeakageFinding::CheckNotRunnable {
                check: "temporal",
                subjects: cohort.subjects.iter().map(|r| r.subject.clone()).collect(),
                missingness: cohort.decision_time.missingness().cloned().unwrap_or(
                    Missingness::NeverMeasured {
                        reason: crate::missingness::UnattemptedReason::Unrecorded,
                    },
                ),
            }];
        };
        let Ok(decision) = Timestamp::parse(raw_decision) else {
            return vec![LeakageFinding::CheckNotRunnable {
                check: "temporal",
                subjects: cohort.subjects.iter().map(|r| r.subject.clone()).collect(),
                missingness: Missingness::TechnicalFailure {
                    assay: "decision_time".into(),
                    detail: format!("`{raw_decision}` is not an RFC 3339 timestamp"),
                },
            }];
        };

        let mut unrecorded = Vec::new();
        let mut unparsed = Vec::new();
        let mut future: BTreeMap<String, String> = BTreeMap::new();
        for record in &cohort.subjects {
            match record.label_source_time.value() {
                None => unrecorded.push(record.subject.clone()),
                Some(raw) => match Timestamp::parse(raw) {
                    Err(_) => unparsed.push(record.subject.clone()),
                    Ok(sourced) => {
                        if sourced > decision {
                            future.insert(record.subject.clone(), raw.clone());
                        }
                    }
                },
            }
        }

        let mut findings = Vec::new();
        if !future.is_empty() {
            findings.push(LeakageFinding::Leak(LeakageWitness::TemporalLeakage {
                decision_time: decision.to_rfc3339(),
                future_label_sources: future,
            }));
        }
        if !unrecorded.is_empty() {
            findings.push(LeakageFinding::CheckNotRunnable {
                check: "temporal",
                subjects: unrecorded,
                missingness: Missingness::NeverMeasured {
                    reason: crate::missingness::UnattemptedReason::Unrecorded,
                },
            });
        }
        if !unparsed.is_empty() {
            findings.push(LeakageFinding::CheckNotRunnable {
                check: "temporal",
                subjects: unparsed,
                missingness: Missingness::TechnicalFailure {
                    assay: "label_source_time".into(),
                    detail: "not an RFC 3339 timestamp".into(),
                },
            });
        }
        findings
    }

    fn preprocessing_findings(&self, cohort: &CohortSplit) -> Vec<LeakageFinding> {
        cohort
            .preprocessing
            .iter()
            .filter(|step| step.fit_over.len() > 1)
            .map(|step| {
                LeakageFinding::Leak(LeakageWitness::PreprocessingLeakage {
                    detail: format!(
                        "`{}` was fit over splits {}",
                        step.name,
                        step.fit_over.join(", ")
                    ),
                })
            })
            .collect()
    }
}

impl Lens for CohortLeakageLens {
    type Evidence = CohortSplit;
    type Witness = LeakageFinding;

    fn declaration(&self) -> LensDeclaration {
        LensDeclaration::new(
            LensId::new(Self::ID),
            "42.10",
            "which dependencies cross a split boundary, and which boundary checks could not run?",
            vec![
                EvidenceRequirement::new("cohort.subjects", "subject split assignments"),
                EvidenceRequirement::new(
                    "cohort.aliases",
                    "every identifier a subject is known by",
                ),
                EvidenceRequirement::new("cohort.site", "the acquisition site of each subject"),
                EvidenceRequirement::new(
                    "cohort.decision_time",
                    "when the training decision was made",
                ),
                EvidenceRequirement::new(
                    "cohort.preprocessing",
                    "which splits each preprocessing fit consumed",
                ),
            ],
            vec![ScopePrecondition::new(
                "cohort",
                "split membership is only meaningful relative to a named cohort",
            )],
            vec![RefusalReason::ScopePreconditionUnmet],
        )
        .expect("42.10 declaration is well formed")
    }

    fn answer(&self, _scope: &ScopeKey, cohort: &CohortSplit) -> LensOutcome<LeakageFinding> {
        if cohort.subjects.is_empty() {
            let gap = crate::grammar::EvidenceGap::new(
                Self::ID,
                vec![crate::grammar::AbsentRequirement {
                    requirement: EvidenceRequirement::new(
                        "cohort.subjects",
                        "subject split assignments",
                    ),
                    missingness: Missingness::NeverMeasured {
                        reason: crate::missingness::UnattemptedReason::NotOrdered,
                    },
                }],
            )
            .expect("one absent requirement is not empty");
            return LensOutcome::EvidenceAbsent(gap);
        }

        let mut findings = self.identity_leaks(cohort);
        findings.extend(self.site_findings(cohort));
        findings.extend(self.temporal_findings(cohort));
        findings.extend(self.preprocessing_findings(cohort));

        let eligible = cohort.subjects.len();
        let coverage = Coverage::complete(Self::ID, eligible, eligible)
            .expect("every subject is examined by every runnable check");
        LensOutcome::Answered {
            witnesses: findings,
            coverage,
        }
    }
}

/// Run the four checks without going through [`crate::run`], for callers that want the findings
/// rather than a sealed report. The sealed path is still the one a gate consumes.
pub fn findings(cohort: &CohortSplit) -> Result<Vec<LeakageFinding>, LensError> {
    let lens = CohortLeakageLens;
    match lens.answer(&ScopeKey::new(), cohort) {
        LensOutcome::Answered { witnesses, .. } => Ok(witnesses),
        LensOutcome::Refused(refusal) => Err(LensError::UndeclaredRefusal {
            lens: CohortLeakageLens::ID.to_string(),
            reason: refusal.reason.as_str(),
        }),
        LensOutcome::EvidenceAbsent(_) => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::{run, ReportOutcome};

    fn subject(subject: &str, split: &str, site: Option<&str>, aliases: &[&str]) -> SubjectRecord {
        SubjectRecord {
            subject: subject.into(),
            split: split.into(),
            aliases: aliases.iter().map(|a| (*a).to_string()).collect(),
            site: match site {
                Some(s) => Recorded::known(s.to_string()),
                None => Recorded::unrecorded(),
            },
            label_source_time: Recorded::unrecorded(),
        }
    }

    fn scope() -> ScopeKey {
        ScopeKey::new().exact("cohort", "C-1")
    }

    #[test]
    fn a_shared_alias_across_splits_produces_a_named_witness_not_a_score() {
        let cohort = CohortSplit::new(vec![
            subject("S001", "train", Some("MGH"), &["ALT-77"]),
            subject("S003", "test", Some("MGH"), &["ALT-77"]),
        ]);
        let found = findings(&cohort).unwrap();
        let leak = found
            .iter()
            .find(|f| f.kind() == "identity_leakage")
            .expect("identity leak detected");
        assert!(leak.sentence().contains("ALT-77"));
        assert!(leak.sentence().contains("S001"));
        assert!(leak.sentence().contains("S003"));
    }

    #[test]
    fn an_unrecorded_site_does_not_certify_the_absence_of_site_leakage() {
        let cohort = CohortSplit::new(vec![
            subject("S001", "train", None, &[]),
            subject("S002", "test", None, &[]),
        ])
        .with_decision_time("2026-01-01T00:00:00Z");
        let found = findings(&cohort).unwrap();
        assert!(found.iter().any(|f| f.kind() == "check_not_runnable"));
        assert!(!found.iter().any(LeakageFinding::is_leak));
        assert_eq!(leakage_status(&found), OracleStatus::Underdetermined);
    }

    #[test]
    fn a_clean_cohort_with_every_check_runnable_is_valid() {
        let mut a = subject("S001", "train", Some("MGH"), &["A1"]);
        let mut b = subject("S002", "test", Some("MGH"), &["B1"]);
        a.label_source_time = Recorded::known("2025-06-01T00:00:00Z".into());
        b.label_source_time = Recorded::known("2025-06-02T00:00:00Z".into());
        let cohort = CohortSplit::new(vec![a, b]).with_decision_time("2026-01-01T00:00:00Z");
        let found = findings(&cohort).unwrap();
        assert!(found.is_empty(), "unexpected findings: {found:?}");
        assert_eq!(leakage_status(&found), OracleStatus::Valid);
    }

    #[test]
    fn one_site_per_split_with_distinct_sites_is_a_confound() {
        let cohort = CohortSplit::new(vec![
            subject("S001", "train", Some("MGH"), &[]),
            subject("S002", "train", Some("MGH"), &[]),
            subject("S003", "test", Some("DFCI"), &[]),
        ])
        .with_decision_time("2026-01-01T00:00:00Z");
        let found = findings(&cohort).unwrap();
        assert!(found.iter().any(|f| f.kind() == "site_leakage"));
        assert_eq!(leakage_status(&found), OracleStatus::Invalid);
    }

    #[test]
    fn sites_shared_between_splits_are_not_a_confound() {
        let cohort = CohortSplit::new(vec![
            subject("S001", "train", Some("MGH"), &[]),
            subject("S002", "test", Some("MGH"), &[]),
        ])
        .with_decision_time("2026-01-01T00:00:00Z");
        let found = findings(&cohort).unwrap();
        assert!(!found.iter().any(|f| f.kind() == "site_leakage"));
    }

    #[test]
    fn a_label_sourced_after_the_decision_time_is_a_temporal_leak() {
        let mut a = subject("S001", "train", Some("MGH"), &[]);
        a.label_source_time = Recorded::known("2026-05-01T00:00:00Z".into());
        let cohort = CohortSplit::new(vec![a]).with_decision_time("2026-01-01T00:00:00Z");
        let found = findings(&cohort).unwrap();
        let leak = found
            .iter()
            .find(|f| f.kind() == "temporal_leakage")
            .expect("temporal leak detected");
        assert!(leak.sentence().contains("2026-01-01"));
    }

    #[test]
    fn an_unrecorded_decision_time_makes_the_temporal_check_unrunnable_not_clean() {
        let mut a = subject("S001", "train", Some("MGH"), &[]);
        a.label_source_time = Recorded::known("2026-05-01T00:00:00Z".into());
        let cohort = CohortSplit::new(vec![a]);
        let found = findings(&cohort).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].kind(), "check_not_runnable");
        assert_eq!(leakage_status(&found), OracleStatus::Underdetermined);
    }

    #[test]
    fn a_preprocessing_fit_spanning_splits_is_a_leak() {
        let cohort = CohortSplit::new(vec![subject("S001", "train", Some("MGH"), &[])])
            .with_decision_time("2026-01-01T00:00:00Z")
            .with_preprocessing(vec![PreprocessingStep {
                name: "zscore".into(),
                fit_over: vec!["train".into(), "test".into()],
            }]);
        let found = findings(&cohort).unwrap();
        assert!(found.iter().any(|f| f.kind() == "preprocessing_leakage"));
    }

    #[test]
    fn an_empty_cohort_reports_absent_evidence_rather_than_a_clean_result() {
        let report = run(&CohortLeakageLens, &scope(), &CohortSplit::new(Vec::new())).unwrap();
        assert!(matches!(report.outcome(), ReportOutcome::EvidenceAbsent(_)));
        assert!(!report.is_answered());
    }

    #[test]
    fn every_leakage_finding_states_itself_as_a_row_and_a_sentence() {
        let cohort = CohortSplit::new(vec![
            subject("S001", "train", None, &["ALT-77"]),
            subject("S003", "test", None, &["ALT-77"]),
        ]);
        let report = run(&CohortLeakageLens, &scope(), &cohort).unwrap();
        for row in report.witnesses() {
            assert_eq!(row.columns.len(), row.cells.len());
            assert!(!row.sentence.is_empty());
            assert!(row.spoken().iter().all(|line| !line.ends_with(": ")));
        }
    }

    #[test]
    fn the_unrunnable_site_check_row_carries_a_missingness_cell_not_a_blank() {
        let cohort = CohortSplit::new(vec![subject("S001", "train", None, &[])])
            .with_decision_time("2026-01-01T00:00:00Z");
        let report = run(&CohortLeakageLens, &scope(), &cohort).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "check_not_runnable")
            .expect("site check reported as unrunnable");
        assert!(row.contains_hole());
    }

    #[test]
    fn the_lens_refuses_when_no_cohort_is_bound() {
        let cohort = CohortSplit::new(vec![subject("S001", "train", Some("MGH"), &[])]);
        let report = run(&CohortLeakageLens, &ScopeKey::new(), &cohort).unwrap();
        match report.outcome() {
            ReportOutcome::Refused(refusal) => {
                assert_eq!(refusal.reason, RefusalReason::ScopePreconditionUnmet);
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
