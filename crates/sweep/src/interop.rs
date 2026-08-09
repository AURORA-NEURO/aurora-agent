//! Runner and benchmark adapters: what a translation does not carry.
//!
//! Implements blueprint 04.03 (Runner and Benchmark Adapters). Its responsibility list contains one
//! phrase that is a predicate rather than an aspiration — "translate environment and verifier
//! semantics **without overstating equivalence**" — and this module is that phrase made checkable.
//!
//! # An equivalence claim is derived, never asserted
//!
//! [`SemanticMap`] holds one [`Mapping`] per external concept, and [`SemanticMap::claim`] computes
//! the strongest claim the map supports. A map containing any [`Mapping::Approximated`] or
//! [`Mapping::Unmapped`] cannot return [`EquivalenceClaim::Equivalent`] — not because a check
//! rejects it, but because the function that would have to produce it never does. 04.03's "unmapped
//! semantics become explicit limitations" is then literal: the limitations are the `Unmapped`
//! entries, carried inside the claim.
//!
//! # Untested is not failed, and a suite with a hole is not a passing suite
//!
//! 04.03's compatibility suite lists eight standard cases. The temptation in every conformance
//! harness is to treat a case that did not run as a case that passed (optimistic) or as one that
//! failed (pessimistic); both destroy the information. [`CaseResult`] therefore has three variants
//! and [`SuiteResult::status`] has three too, with [`SuiteStatus::Incomplete`] sitting between
//! passing and failing. Only [`SuiteStatus::Passing`] promotes a framework version out of preview.
//!
//! This is the same shape as [`crate::conform`]'s provider capability cards, in a different
//! currency, and both are the same shape as the workspace's rule that zero influence and unknown
//! influence must not share a representation.
//!
//! # The other runner's logs stay the source of truth
//!
//! 04.03: "When another runner executes the task, its native logs remain linked evidence. PRISM
//! does not rewrite them into a falsely canonical story." [`ExternalResult::new`] refuses to build
//! an execution-adapter result with no native-log reference, so the link cannot be forgotten into
//! existence.
//!
//! # What is not implemented
//!
//! No adapters. Harbor, BenchFlow and Inspect are named in the blueprint and integrated nowhere:
//! this workspace is offline and cannot link them, and — the stronger reason — a second Rust
//! reading of another runner's log format would surface its disagreements as benchmark results
//! rather than as parser bugs. `bioprism-adapter` makes the same call at the DICOM boundary. What
//! is here is the contract those adapters would have to satisfy.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};

/// 04.03's four adapter directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterDirection {
    /// Reads historical runs.
    ImportOnly,
    /// Submits experiments and streams events.
    Execution,
    /// Translates benchmark tasks.
    Pack,
    /// Packages cells for another framework.
    Export,
}

impl AdapterDirection {
    /// Whether the other system, rather than this one, ran the task.
    pub fn foreign_execution(self) -> bool {
        matches!(self, AdapterDirection::ImportOnly | AdapterDirection::Execution)
    }
}

/// What became of one external concept under translation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Mapping {
    /// Carried across with its meaning intact.
    Mapped { to: String },
    /// Carried across with a stated caveat. The caveat is required.
    Approximated { to: String, caveat: String },
    /// Not carried across. 04.03's "explicit limitation".
    Unmapped { limitation: String },
}

impl Mapping {
    pub fn mapped(to: impl Into<String>) -> Self {
        Mapping::Mapped { to: to.into() }
    }

    pub fn approximated(
        to: impl Into<String>,
        caveat: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let caveat = caveat.into();
        require_nonempty(&caveat, "Mapping::approximated", "caveat")?;
        Ok(Mapping::Approximated { to: to.into(), caveat })
    }

    pub fn unmapped(limitation: impl Into<String>) -> Result<Self, SweepError> {
        let limitation = limitation.into();
        require_nonempty(&limitation, "Mapping::unmapped", "limitation")?;
        Ok(Mapping::Unmapped { limitation })
    }
}

/// The strongest thing an adapter may say about its translation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "claim")]
pub enum EquivalenceClaim {
    /// Every declared concept mapped cleanly.
    Equivalent,
    /// Some concepts were approximated or dropped. Both lists travel with the claim.
    Partial { caveats: Vec<String>, limitations: Vec<String> },
    /// Concepts were declared and never given a mapping. Not the same as `Partial`: a partial
    /// translation was audited, this one was not finished.
    Incomplete { unaddressed: Vec<String> },
}

/// 04.03's semantic map: how each external concept lands in PRISM's fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticMap {
    declared: Vec<String>,
    entries: BTreeMap<String, Mapping>,
}

impl SemanticMap {
    /// Declare the concepts this adapter must account for.
    ///
    /// 04.03 lists "task inputs, images, setup scripts, tools, timeouts, graders, artifacts, and
    /// retries" as the concepts an adapter documents. That list is the blueprint's example rather
    /// than a closed set, so it is not hard-coded here; callers declare their own and the map
    /// checks against what they declared.
    pub fn declaring(mut self, concept: impl Into<String>) -> Self {
        self.declared.push(concept.into());
        self
    }

    pub fn mapping(mut self, concept: impl Into<String>, mapping: Mapping) -> Self {
        self.entries.insert(concept.into(), mapping);
        self
    }

    /// Declared concepts with no entry.
    pub fn unaddressed(&self) -> Vec<String> {
        self.declared
            .iter()
            .filter(|c| !self.entries.contains_key(*c))
            .cloned()
            .collect()
    }

    /// The strongest claim this map supports.
    ///
    /// There is no argument and no override. An adapter cannot pass in the claim it would like.
    pub fn claim(&self) -> EquivalenceClaim {
        let unaddressed = self.unaddressed();
        if !unaddressed.is_empty() {
            return EquivalenceClaim::Incomplete { unaddressed };
        }
        let mut caveats = Vec::new();
        let mut limitations = Vec::new();
        for mapping in self.entries.values() {
            match mapping {
                Mapping::Mapped { .. } => {}
                Mapping::Approximated { caveat, .. } => caveats.push(caveat.clone()),
                Mapping::Unmapped { limitation } => limitations.push(limitation.clone()),
            }
        }
        if caveats.is_empty() && limitations.is_empty() {
            EquivalenceClaim::Equivalent
        } else {
            EquivalenceClaim::Partial { caveats, limitations }
        }
    }
}

/// 04.03's eight compatibility-suite cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityCase {
    SuccessfulCompletion,
    Timeout,
    ToolError,
    FilesystemChanges,
    NetworkControls,
    GraderFailure,
    Cancellation,
    ArtifactRetrieval,
}

impl CompatibilityCase {
    pub const ALL: [CompatibilityCase; 8] = [
        CompatibilityCase::SuccessfulCompletion,
        CompatibilityCase::Timeout,
        CompatibilityCase::ToolError,
        CompatibilityCase::FilesystemChanges,
        CompatibilityCase::NetworkControls,
        CompatibilityCase::GraderFailure,
        CompatibilityCase::Cancellation,
        CompatibilityCase::ArtifactRetrieval,
    ];
}

/// What one compatibility case established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseResult {
    Pass,
    Fail,
    /// Never executed. Not a pass and not a failure.
    NotRun,
}

/// The roll-up of a compatibility suite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuiteStatus {
    Passing,
    /// Every case that ran passed, but some did not run.
    Incomplete,
    Failing,
}

/// One run of the compatibility suite against one framework version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuiteResult {
    results: BTreeMap<String, CaseResult>,
}

impl SuiteResult {
    /// Every case starts [`CaseResult::NotRun`]. That default is the honest one: a suite nobody
    /// ran has run no cases.
    pub fn new() -> Self {
        let results = CompatibilityCase::ALL
            .into_iter()
            .map(|c| (format!("{c:?}"), CaseResult::NotRun))
            .collect();
        SuiteResult { results }
    }

    pub fn recording(mut self, case: CompatibilityCase, result: CaseResult) -> Self {
        self.results.insert(format!("{case:?}"), result);
        self
    }

    pub fn result(&self, case: CompatibilityCase) -> CaseResult {
        self.results.get(&format!("{case:?}")).copied().unwrap_or(CaseResult::NotRun)
    }

    pub fn not_run(&self) -> Vec<CompatibilityCase> {
        CompatibilityCase::ALL
            .into_iter()
            .filter(|c| self.result(*c) == CaseResult::NotRun)
            .collect()
    }

    /// Failing dominates incomplete; incomplete dominates passing.
    pub fn status(&self) -> SuiteStatus {
        let mut incomplete = false;
        for case in CompatibilityCase::ALL {
            match self.result(case) {
                CaseResult::Fail => return SuiteStatus::Failing,
                CaseResult::NotRun => incomplete = true,
                CaseResult::Pass => {}
            }
        }
        if incomplete {
            SuiteStatus::Incomplete
        } else {
            SuiteStatus::Passing
        }
    }
}

impl Default for SuiteResult {
    fn default() -> Self {
        SuiteResult::new()
    }
}

/// How much of a framework version an adapter supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// The compatibility suite passed on this version.
    Tested,
    /// Declared but not yet demonstrated. 04.03: "A new upstream version enters preview until
    /// conformance tests pass."
    Preview,
    /// Not declared.
    Unsupported,
}

/// Which framework versions an adapter claims, and on what basis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionMatrix {
    entries: BTreeMap<String, Support>,
}

impl VersionMatrix {
    pub fn new() -> Self {
        VersionMatrix::default()
    }

    /// Declaring a version puts it in preview, never straight into tested.
    pub fn declare(&mut self, framework: &str, version: &str) {
        self.entries.insert(Self::key(framework, version), Support::Preview);
    }

    /// Promote a version out of preview by supplying a passing suite.
    ///
    /// An `Incomplete` suite does not promote: 04.03's gate is that conformance tests pass, and a
    /// suite with cases that never ran has not established that.
    pub fn promote(
        &mut self,
        framework: &str,
        version: &str,
        suite: &SuiteResult,
    ) -> Result<(), SweepError> {
        let key = Self::key(framework, version);
        if !self.entries.contains_key(&key) {
            return Err(SweepError::UndeclaredPrecondition {
                operation: "VersionMatrix::promote",
                declaration: "the version must be declared before it can be promoted",
            });
        }
        match suite.status() {
            SuiteStatus::Passing => {
                self.entries.insert(key, Support::Tested);
                Ok(())
            }
            status => Err(SweepError::Unproven {
                subject: key,
                claim: "tested".to_string(),
                state: match status {
                    SuiteStatus::Incomplete => "incomplete",
                    SuiteStatus::Failing => "failing",
                    SuiteStatus::Passing => unreachable!("handled above"),
                },
            }),
        }
    }

    pub fn support(&self, framework: &str, version: &str) -> Support {
        self.entries
            .get(&Self::key(framework, version))
            .copied()
            .unwrap_or(Support::Unsupported)
    }

    fn key(framework: &str, version: &str) -> String {
        format!("{framework}@{version}")
    }
}

/// A result produced by another runner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalResult {
    pub direction: AdapterDirection,
    pub framework: String,
    /// A reference to the other runner's own log. Required whenever the other runner executed.
    pub native_log: Option<String>,
    pub claim: EquivalenceClaim,
}

impl ExternalResult {
    /// Build a result, requiring the native-log link where the other runner did the work.
    pub fn new(
        direction: AdapterDirection,
        framework: impl Into<String>,
        native_log: Option<String>,
        map: &SemanticMap,
    ) -> Result<Self, SweepError> {
        if direction.foreign_execution() && native_log.as_deref().unwrap_or("").trim().is_empty() {
            return Err(SweepError::UndeclaredPrecondition {
                operation: "ExternalResult::new",
                declaration: "a reference to the external runner's native log",
            });
        }
        Ok(ExternalResult {
            direction,
            framework: framework.into(),
            native_log,
            claim: map.claim(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_map() -> SemanticMap {
        SemanticMap::default()
            .declaring("timeout")
            .declaring("grader")
            .mapping("timeout", Mapping::mapped("budget.wall_millis"))
            .mapping("grader", Mapping::mapped("evaluator"))
    }

    fn passing_suite() -> SuiteResult {
        CompatibilityCase::ALL
            .into_iter()
            .fold(SuiteResult::new(), |s, c| s.recording(c, CaseResult::Pass))
    }

    #[test]
    fn a_map_with_every_concept_cleanly_mapped_claims_equivalence() {
        assert_eq!(clean_map().claim(), EquivalenceClaim::Equivalent);
    }

    #[test]
    fn one_approximation_downgrades_the_claim_and_carries_its_caveat() {
        let map = clean_map().mapping(
            "timeout",
            Mapping::approximated("budget.wall_millis", "the runner's timeout excludes setup")
                .unwrap(),
        );
        match map.claim() {
            EquivalenceClaim::Partial { caveats, limitations } => {
                assert_eq!(caveats.len(), 1);
                assert!(caveats[0].contains("excludes setup"));
                assert!(limitations.is_empty());
            }
            other => panic!("expected Partial, got {other:?}"),
        }
    }

    #[test]
    fn an_unmapped_concept_becomes_an_explicit_limitation() {
        let map = clean_map()
            .declaring("retries")
            .mapping("retries", Mapping::unmapped("PRISM has no retry semantics").unwrap());
        match map.claim() {
            EquivalenceClaim::Partial { limitations, .. } => {
                assert_eq!(limitations, ["PRISM has no retry semantics"]);
            }
            other => panic!("expected Partial, got {other:?}"),
        }
    }

    #[test]
    fn a_declared_concept_with_no_entry_makes_the_map_incomplete_not_partial() {
        let map = clean_map().declaring("artifacts");
        assert_eq!(
            map.claim(),
            EquivalenceClaim::Incomplete { unaddressed: vec!["artifacts".into()] }
        );
    }

    #[test]
    fn an_approximation_without_a_caveat_is_refused() {
        assert!(Mapping::approximated("x", "").is_err());
        assert!(Mapping::unmapped("   ").is_err());
    }

    #[test]
    fn a_fresh_suite_has_run_nothing_and_is_incomplete() {
        let suite = SuiteResult::new();
        assert_eq!(suite.status(), SuiteStatus::Incomplete);
        assert_eq!(suite.not_run().len(), 8);
    }

    #[test]
    fn a_case_that_never_ran_is_not_a_case_that_failed() {
        let suite = passing_suite().recording(CompatibilityCase::Cancellation, CaseResult::NotRun);
        assert_eq!(suite.status(), SuiteStatus::Incomplete);
        let failing = passing_suite().recording(CompatibilityCase::Cancellation, CaseResult::Fail);
        assert_eq!(failing.status(), SuiteStatus::Failing);
    }

    #[test]
    fn a_failing_case_dominates_a_case_that_never_ran() {
        let suite = SuiteResult::new()
            .recording(CompatibilityCase::Timeout, CaseResult::Fail);
        assert_eq!(suite.status(), SuiteStatus::Failing);
    }

    #[test]
    fn a_declared_framework_version_starts_in_preview() {
        let mut matrix = VersionMatrix::new();
        matrix.declare("inspect", "0.9.0");
        assert_eq!(matrix.support("inspect", "0.9.0"), Support::Preview);
        assert_eq!(matrix.support("inspect", "0.8.0"), Support::Unsupported);
    }

    #[test]
    fn an_incomplete_suite_cannot_promote_a_version_out_of_preview() {
        let mut matrix = VersionMatrix::new();
        matrix.declare("inspect", "0.9.0");
        let partial =
            passing_suite().recording(CompatibilityCase::NetworkControls, CaseResult::NotRun);
        let err = matrix.promote("inspect", "0.9.0", &partial).unwrap_err();
        assert!(matches!(err, SweepError::Unproven { state: "incomplete", .. }));
        assert_eq!(matrix.support("inspect", "0.9.0"), Support::Preview);
    }

    #[test]
    fn a_passing_suite_promotes_a_declared_version() {
        let mut matrix = VersionMatrix::new();
        matrix.declare("inspect", "0.9.0");
        matrix.promote("inspect", "0.9.0", &passing_suite()).unwrap();
        assert_eq!(matrix.support("inspect", "0.9.0"), Support::Tested);
    }

    #[test]
    fn an_undeclared_version_cannot_be_promoted_even_by_a_passing_suite() {
        let mut matrix = VersionMatrix::new();
        assert!(matrix.promote("harbor", "2.0.0", &passing_suite()).is_err());
    }

    #[test]
    fn an_execution_adapter_result_without_a_native_log_reference_is_refused() {
        let err = ExternalResult::new(
            AdapterDirection::Execution,
            "harbor",
            None,
            &clean_map(),
        )
        .unwrap_err();
        assert!(matches!(err, SweepError::UndeclaredPrecondition { .. }));
        assert!(ExternalResult::new(
            AdapterDirection::Execution,
            "harbor",
            Some("s3://logs/run-1".into()),
            &clean_map(),
        )
        .is_ok());
    }

    #[test]
    fn an_export_adapter_needs_no_native_log_because_prism_ran_the_task() {
        assert!(ExternalResult::new(AdapterDirection::Export, "inspect", None, &clean_map()).is_ok());
        assert!(!AdapterDirection::Export.foreign_execution());
        assert!(AdapterDirection::ImportOnly.foreign_execution());
    }
}
