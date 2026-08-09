//! Progressive rendering and usability release gates — blueprint 42.30 and 42.31.
//!
//! 42.31 says to "gate features on real task success, semantic correctness, accessibility,
//! provenance, and privacy — not visual polish alone". That is a paragraph, and a paragraph is
//! not a gate. [`ReleaseGate::evaluate`] is: a pure predicate over a set of [`LensReport`]s that
//! returns the concrete reasons a release is blocked, each one a witness rather than a score.
//!
//! Five checks, and the ordering of the first two is the point:
//!
//! 1. **A required lens with no report is [`GateBlock::LensNotRun`]**, which is a different block
//!    from any failure. "Nobody ran the leakage lens" must never read as "the leakage lens found
//!    nothing", and the gate output keeps them in separate variants so no consumer can merge them.
//! 2. **An answer with absent evidence blocks.** A gate cannot pass on a lens that could not see
//!    what it needed. This is the same refusal as 43.26's: one unknown group voids sufficiency.
//! 3. **A partial answer blocks unless partial answers are explicitly permitted** — 42.30. The
//!    permission is a field on the gate, so allowing incomplete coverage is a recorded decision
//!    rather than an oversight.
//! 4. **A refusal blocks unless its reason is tolerated.** A lens declining to answer is legitimate
//!    and still not evidence of a passing state.
//! 5. **Accessibility.** Every witness row must speak: no field may be empty text, and columns
//!    must match cells. [`crate::run`] already rejects ragged rows, so this catches the one
//!    remaining hole — a cell holding an empty string, which is a blank cell in disguise and
//!    exactly what 42.27 exists to prevent.
//!
//! # What is deliberately not gated
//!
//! **Privacy.** 42.31 names it and the enforcement lives in the policy fibers of 43.33; a gate
//! here could only check that a lens *reported* a policy boundary, which is what
//! [`RefusalReason::PolicyWithheld`] already carries. **Task success with real users**, which is a
//! measurement no type can make. **Latency, first-useful-view budgets and layout caching**
//! (42.30's performance half): there is no renderer, no viewport and no clock in this crate, so
//! any number here would be fabricated. What survives of 42.30 is the part that is about meaning
//! rather than speed — completeness as a first-class field — and that is enforced in
//! [`Coverage`](crate::Coverage) rather than here.

use crate::grammar::{Completeness, LensId, LensReport, RefusalReason, ReportOutcome};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One concrete reason a release is blocked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "block", rename_all = "snake_case")]
pub enum GateBlock {
    /// A required lens produced no report at all. Not a failure — an absence.
    LensNotRun { lens: String },
    /// A lens could not see the evidence it requires.
    EvidenceAbsent {
        lens: String,
        requirements: Vec<String>,
    },
    /// A lens answered over part of its eligible input.
    AnswerIncomplete {
        lens: String,
        examined: usize,
        eligible: usize,
        pending: Vec<String>,
    },
    /// A lens refused, for a reason this gate does not tolerate.
    RefusalNotTolerated { lens: String, reason: String },
    /// A lens produced a finding of a kind this gate treats as blocking.
    BlockingFinding {
        lens: String,
        kind: String,
        sentence: String,
    },
    /// A witness row carried a field that speaks as nothing.
    UnspeakableWitness {
        lens: String,
        kind: String,
        column: String,
    },
}

impl GateBlock {
    pub fn lens(&self) -> &str {
        match self {
            GateBlock::LensNotRun { lens }
            | GateBlock::EvidenceAbsent { lens, .. }
            | GateBlock::AnswerIncomplete { lens, .. }
            | GateBlock::RefusalNotTolerated { lens, .. }
            | GateBlock::BlockingFinding { lens, .. }
            | GateBlock::UnspeakableWitness { lens, .. } => lens,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            GateBlock::LensNotRun { .. } => "lens_not_run",
            GateBlock::EvidenceAbsent { .. } => "evidence_absent",
            GateBlock::AnswerIncomplete { .. } => "answer_incomplete",
            GateBlock::RefusalNotTolerated { .. } => "refusal_not_tolerated",
            GateBlock::BlockingFinding { .. } => "blocking_finding",
            GateBlock::UnspeakableWitness { .. } => "unspeakable_witness",
        }
    }

    /// A sentence a reader can act on, in the same non-visual register as a witness.
    pub fn sentence(&self) -> String {
        match self {
            GateBlock::LensNotRun { lens } => format!(
                "`{lens}` is required by this gate and was never run; nothing is known about what \
                 it would have found"
            ),
            GateBlock::EvidenceAbsent { lens, requirements } => format!(
                "`{lens}` could not run: {} required input(s) absent ({})",
                requirements.len(),
                requirements.join(", ")
            ),
            GateBlock::AnswerIncomplete {
                lens,
                examined,
                eligible,
                pending,
            } => format!(
                "`{lens}` answered over {examined} of {eligible} eligible unit(s); not reached: {}",
                pending.join(", ")
            ),
            GateBlock::RefusalNotTolerated { lens, reason } => {
                format!("`{lens}` refused with `{reason}`, which this gate does not tolerate")
            }
            GateBlock::BlockingFinding {
                lens,
                kind,
                sentence,
            } => format!("`{lens}` produced a blocking `{kind}`: {sentence}"),
            GateBlock::UnspeakableWitness { lens, kind, column } => format!(
                "`{lens}` produced a `{kind}` whose `{column}` field speaks as nothing, so the \
                 non-visual rendition is not equivalent to the finding"
            ),
        }
    }
}

/// The result of applying a gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateOutcome {
    pub blocks: Vec<GateBlock>,
    pub lenses_required: usize,
    pub reports_considered: usize,
}

impl GateOutcome {
    pub fn passed(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Blocks that mean "nobody checked", as opposed to "checked and found a problem".
    ///
    /// Kept separate because a release conversation goes differently depending on which it is,
    /// and a single count would hide the difference.
    pub fn unchecked(&self) -> Vec<&GateBlock> {
        self.blocks
            .iter()
            .filter(|b| {
                matches!(
                    b,
                    GateBlock::LensNotRun { .. }
                        | GateBlock::EvidenceAbsent { .. }
                        | GateBlock::AnswerIncomplete { .. }
                )
            })
            .collect()
    }

    /// Blocks that mean a lens looked and found something.
    pub fn established(&self) -> Vec<&GateBlock> {
        self.blocks
            .iter()
            .filter(|b| matches!(b, GateBlock::BlockingFinding { .. }))
            .collect()
    }

    pub fn spoken(&self) -> Vec<String> {
        let mut lines = vec![if self.passed() {
            format!(
                "gate passed over {} required lens/lenses",
                self.lenses_required
            )
        } else {
            format!("gate blocked by {} finding(s)", self.blocks.len())
        }];
        lines.extend(self.blocks.iter().map(GateBlock::sentence));
        lines
    }
}

/// An executable release gate over a set of lenses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseGate {
    /// Lenses that must have a report. A missing one is [`GateBlock::LensNotRun`].
    pub required: Vec<LensId>,
    /// Whether a partial answer is acceptable. Default `false`: 42.30's failure mode is treating
    /// an unfinished view as finished, and the safe default is to say so.
    #[serde(default)]
    pub allow_partial: bool,
    /// Refusal reasons this gate accepts without blocking.
    #[serde(default)]
    pub tolerated_refusals: Vec<RefusalReason>,
    /// Witness kinds that block a release outright — `identity_leakage`, say.
    #[serde(default)]
    pub blocking_findings: Vec<String>,
}

impl ReleaseGate {
    pub fn requiring(required: Vec<LensId>) -> Self {
        ReleaseGate {
            required,
            allow_partial: false,
            tolerated_refusals: Vec::new(),
            blocking_findings: Vec::new(),
        }
    }

    pub fn tolerating(mut self, reasons: Vec<RefusalReason>) -> Self {
        self.tolerated_refusals = reasons;
        self
    }

    pub fn blocking_on(mut self, kinds: Vec<&str>) -> Self {
        self.blocking_findings = kinds.iter().map(|k| (*k).to_string()).collect();
        self
    }

    pub fn permitting_partial(mut self) -> Self {
        self.allow_partial = true;
        self
    }

    /// Apply the gate. Pure, deterministic, order-stable: blocks come out in required-lens order,
    /// then in report order within a lens.
    pub fn evaluate(&self, reports: &[LensReport]) -> GateOutcome {
        let mut blocks = Vec::new();
        let reported: BTreeSet<&str> = reports.iter().map(|r| r.lens().as_str()).collect();

        for required in &self.required {
            if !reported.contains(required.as_str()) {
                blocks.push(GateBlock::LensNotRun {
                    lens: required.as_str().to_string(),
                });
            }
        }

        for report in reports {
            let lens = report.lens().as_str().to_string();
            match report.outcome() {
                ReportOutcome::EvidenceAbsent(gap) => blocks.push(GateBlock::EvidenceAbsent {
                    lens: lens.clone(),
                    requirements: gap
                        .absent()
                        .iter()
                        .map(|a| a.requirement.key.clone())
                        .collect(),
                }),
                ReportOutcome::Refused(refusal) => {
                    if !self.tolerated_refusals.contains(&refusal.reason) {
                        blocks.push(GateBlock::RefusalNotTolerated {
                            lens: lens.clone(),
                            reason: refusal.reason.as_str().to_string(),
                        });
                    }
                }
                ReportOutcome::Answered {
                    witnesses,
                    coverage,
                } => {
                    if !self.allow_partial {
                        if let Completeness::Partial { examined, eligible } =
                            coverage.completeness()
                        {
                            blocks.push(GateBlock::AnswerIncomplete {
                                lens: lens.clone(),
                                examined,
                                eligible,
                                pending: coverage
                                    .pending()
                                    .iter()
                                    .map(|p| p.region.clone())
                                    .collect(),
                            });
                        }
                    }
                    for row in witnesses {
                        if self.blocking_findings.contains(&row.kind) {
                            blocks.push(GateBlock::BlockingFinding {
                                lens: lens.clone(),
                                kind: row.kind.clone(),
                                sentence: row.sentence.clone(),
                            });
                        }
                        for (column, cell) in row.columns.iter().zip(&row.cells) {
                            if cell.spoken().trim().is_empty() {
                                blocks.push(GateBlock::UnspeakableWitness {
                                    lens: lens.clone(),
                                    kind: row.kind.clone(),
                                    column: column.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        GateOutcome {
            blocks,
            lenses_required: self.required.len(),
            reports_considered: reports.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::run;
    use crate::leakage::{CohortLeakageLens, CohortSplit, SubjectRecord};
    use crate::missingness::Recorded;
    use bioprism_scope::ScopeKey;

    fn subject(subject: &str, split: &str, site: Option<&str>, alias: &str) -> SubjectRecord {
        SubjectRecord {
            subject: subject.into(),
            split: split.into(),
            aliases: vec![alias.to_string()],
            site: match site {
                Some(s) => Recorded::known(s.to_string()),
                None => Recorded::unrecorded(),
            },
            label_source_time: Recorded::known("2025-01-01T00:00:00Z".into()),
        }
    }

    fn cohort_scope() -> ScopeKey {
        ScopeKey::new().exact("cohort", "C-1")
    }

    fn clean_report() -> LensReport {
        let cohort = CohortSplit::new(vec![
            subject("S001", "train", Some("MGH"), "A1"),
            subject("S002", "test", Some("MGH"), "B1"),
        ])
        .with_decision_time("2026-01-01T00:00:00Z");
        run(&CohortLeakageLens, &cohort_scope(), &cohort).unwrap()
    }

    fn leaking_report() -> LensReport {
        let cohort = CohortSplit::new(vec![
            subject("S001", "train", Some("MGH"), "ALT-77"),
            subject("S002", "test", Some("MGH"), "ALT-77"),
        ])
        .with_decision_time("2026-01-01T00:00:00Z");
        run(&CohortLeakageLens, &cohort_scope(), &cohort).unwrap()
    }

    fn gate() -> ReleaseGate {
        ReleaseGate::requiring(vec![LensId::new(CohortLeakageLens::ID)])
            .blocking_on(vec!["identity_leakage", "site_leakage"])
    }

    #[test]
    fn a_gate_over_a_clean_report_passes() {
        let outcome = gate().evaluate(&[clean_report()]);
        assert!(outcome.passed(), "{:?}", outcome.blocks);
    }

    #[test]
    fn a_required_lens_that_never_ran_blocks_and_is_not_called_a_failure() {
        let outcome = gate().evaluate(&[]);
        assert!(!outcome.passed());
        assert_eq!(outcome.blocks.len(), 1);
        assert_eq!(outcome.blocks[0].kind(), "lens_not_run");
        assert_eq!(outcome.unchecked().len(), 1);
        assert!(outcome.established().is_empty());
    }

    #[test]
    fn a_lens_not_run_and_a_lens_that_found_a_leak_are_different_blocks() {
        let not_run = gate().evaluate(&[]);
        let leaked = gate().evaluate(&[leaking_report()]);
        assert_ne!(not_run.blocks[0].kind(), leaked.blocks[0].kind());
        assert_eq!(leaked.established().len(), 1);
        assert!(leaked.unchecked().is_empty());
    }

    #[test]
    fn a_blocking_finding_reaches_the_gate_output_with_its_own_sentence() {
        let outcome = gate().evaluate(&[leaking_report()]);
        assert!(!outcome.passed());
        assert!(outcome.blocks[0].sentence().contains("ALT-77"));
    }

    #[test]
    fn a_finding_kind_the_gate_does_not_block_on_does_not_block() {
        let permissive = ReleaseGate::requiring(vec![LensId::new(CohortLeakageLens::ID)]);
        assert!(permissive.evaluate(&[leaking_report()]).passed());
    }

    #[test]
    fn absent_evidence_blocks_a_gate_because_nobody_checked_is_not_a_pass() {
        let report = run(
            &CohortLeakageLens,
            &cohort_scope(),
            &CohortSplit::new(Vec::new()),
        )
        .unwrap();
        let outcome = gate().evaluate(&[report]);
        assert_eq!(outcome.blocks[0].kind(), "evidence_absent");
        assert!(outcome.blocks[0].sentence().contains("cohort.subjects"));
    }

    #[test]
    fn a_refusal_blocks_unless_the_gate_tolerates_its_reason() {
        let cohort = CohortSplit::new(vec![subject("S001", "train", Some("MGH"), "A1")]);
        let report = run(&CohortLeakageLens, &ScopeKey::new(), &cohort).unwrap();
        assert_eq!(
            gate().evaluate(std::slice::from_ref(&report)).blocks[0].kind(),
            "refusal_not_tolerated"
        );
        let tolerant = gate().tolerating(vec![RefusalReason::ScopePreconditionUnmet]);
        assert!(tolerant.evaluate(&[report]).passed());
    }

    #[test]
    fn a_partial_answer_blocks_unless_the_gate_permits_partial_coverage() {
        use crate::anytime::{AnytimeCurveLens, AnytimeEvaluation, CurvePoint, Stratum};
        let evaluation = AnytimeEvaluation::new(
            "eval-1",
            vec![Stratum::new("imaging", 10), Stratum::new("molecular", 10)],
        )
        .with_points(vec![CurvePoint::new("imaging", 5, 4)]);
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        let strict = ReleaseGate::requiring(vec![LensId::new(AnytimeCurveLens::ID)]);
        let outcome = strict.evaluate(std::slice::from_ref(&report));
        assert_eq!(outcome.blocks[0].kind(), "answer_incomplete");
        assert!(outcome.blocks[0].sentence().contains("molecular"));
        assert!(strict.permitting_partial().evaluate(&[report]).passed());
    }

    #[test]
    fn a_gate_output_speaks_its_blocks_without_a_rendering() {
        let outcome = gate().evaluate(&[leaking_report()]);
        let spoken = outcome.spoken();
        assert!(spoken[0].contains("blocked by 1"));
        assert!(spoken.len() == 1 + outcome.blocks.len());
    }

    #[test]
    fn an_empty_gate_over_no_reports_passes_and_says_how_little_it_checked() {
        let outcome = ReleaseGate::default().evaluate(&[]);
        assert!(outcome.passed());
        assert_eq!(outcome.lenses_required, 0);
        assert_eq!(outcome.reports_considered, 0);
    }

    #[test]
    fn a_witness_field_that_speaks_as_nothing_fails_the_accessibility_check() {
        use crate::grammar::{Coverage, EvidenceRequirement, Lens, LensDeclaration, LensOutcome};
        use crate::nonvisual::{Cell, Witness};

        struct SilentFinding;
        impl Witness for SilentFinding {
            fn kind(&self) -> &'static str {
                "silent"
            }
            fn columns(&self) -> &'static [&'static str] {
                &["label"]
            }
            fn cells(&self) -> Vec<Cell> {
                vec![Cell::text("")]
            }
            fn sentence(&self) -> String {
                "a finding with nothing to say".into()
            }
        }

        struct SilentLens;
        impl Lens for SilentLens {
            type Evidence = ();
            type Witness = SilentFinding;
            fn declaration(&self) -> LensDeclaration {
                LensDeclaration::new(
                    LensId::new("silent"),
                    "42.27",
                    "can a witness say nothing?",
                    vec![EvidenceRequirement::new("none", "none")],
                    Vec::new(),
                    Vec::new(),
                )
                .unwrap()
            }
            fn answer(&self, _s: &ScopeKey, _e: &()) -> LensOutcome<SilentFinding> {
                LensOutcome::Answered {
                    witnesses: vec![SilentFinding],
                    coverage: Coverage::complete("silent", 1, 1).unwrap(),
                }
            }
        }

        let report = run(&SilentLens, &ScopeKey::new(), &()).unwrap();
        let outcome = ReleaseGate::requiring(vec![LensId::new("silent")]).evaluate(&[report]);
        assert_eq!(outcome.blocks[0].kind(), "unspeakable_witness");
        assert!(outcome.blocks[0].sentence().contains("not equivalent"));
    }

    #[test]
    fn gate_evaluation_is_deterministic() {
        let reports = vec![leaking_report(), clean_report()];
        assert_eq!(gate().evaluate(&reports), gate().evaluate(&reports));
    }
}
