//! Red-team findings and the vulnerability lifecycle, advancing on operator epochs.
//!
//! Implements blueprint 13.21 (security testing, red team and bug bounty) and 13.24 (responsible
//! disclosure and vulnerability lifecycle).
//!
//! # No clock
//!
//! 13.24's "Lifecycle metrics" are all durations — time to acknowledge, to triage, to patch. This
//! module has no clock, for the reason `bioprism-governance` has none: a lifecycle that advances on
//! wall time is not reproducible, and a timeline an attacker can influence is a timeline an
//! attacker can hide in. Every transition carries an operator-supplied epoch,
//! [`Vulnerability::advance`] refuses one that does not move forward, and
//! [`Vulnerability::elapsed_in_stage`] measures in epochs. What an epoch means — a day, a release,
//! a standup — is the deployment's decision and nothing here depends on it.
//!
//! # The ladder is a ladder
//!
//! `reported → triaged → fixed → disclosed`, one rung at a time. Skipping triage would mean
//! disclosing something nobody classified; skipping the fix would mean publishing a live
//! vulnerability, which is the thing coordinated disclosure exists to prevent. Both are
//! [`SafetyError::LifecycleViolation`]. [`Stage::Withdrawn`] and [`Stage::Duplicate`] are terminal
//! and reachable from anywhere before disclosure, because a report can turn out to be neither.
//!
//! # A finding nobody reproduced is not a regression test
//!
//! 13.21 says each confirmed class becomes "a minimized security microbenchmark and protected CI
//! sentinel". [`Finding::into_regression_cell`] requires [`FindingStatus::Confirmed`] and returns
//! [`SafetyError::UnconfirmedFinding`] otherwise. Installing a sentinel for an unreproduced report
//! adds a test that may assert nothing, and a green suite that means less than it did.
//!
//! # Embargo hides the exploit, never the existence
//!
//! [`RegressionCell::embargoed`] withholds the reproduction detail. It does not and cannot withhold
//! that the cell exists — [`Corpus::sentinel_count`] counts embargoed cells too, and
//! [`crate::release`] carries the typed refusal for the case where someone tries to use safety
//! review to suppress the existence of a weakness.
//!
//! # What is deliberately not implemented
//!
//! * **No intake, no triage queue, no notification, no bounty payment.** 13.24's process is people.
//! * **No fuzzers and no adversarial corpus.** 13.21's automated suites — escape probes, parser
//!   fuzzing, exfiltration attempts — need something to execute against. [`Corpus`] is a registry
//!   of what exists, not a runner.
//! * **No CVE assignment, no severity calculator.** [`Severity`] is an operator's judgement with
//!   the three impact axes 13.24 names recorded beside it, so a reader can see what the judgement
//!   was based on rather than a single letter.
//! * **No embargo enforcement.** An embargoed cell's detail is `Option::None` in a struct. Nothing
//!   stops a caller printing what it does hold.

use crate::error::SafetyError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Where a report is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Reported,
    Triaged,
    Fixed,
    Disclosed,
    /// The reporter withdrew it, or it turned out not to be a vulnerability.
    Withdrawn,
    /// Already tracked under another identifier.
    Duplicate,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Reported => "reported",
            Stage::Triaged => "triaged",
            Stage::Fixed => "fixed",
            Stage::Disclosed => "disclosed",
            Stage::Withdrawn => "withdrawn",
            Stage::Duplicate => "duplicate",
        }
    }

    /// Position on the main ladder; `None` for the terminal outcomes.
    pub fn rung(self) -> Option<u8> {
        match self {
            Stage::Reported => Some(0),
            Stage::Triaged => Some(1),
            Stage::Fixed => Some(2),
            Stage::Disclosed => Some(3),
            Stage::Withdrawn | Stage::Duplicate => None,
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Stage::Disclosed | Stage::Withdrawn | Stage::Duplicate)
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An operator's severity judgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    /// 13.24 requires independent verification of high-severity fixes.
    pub fn requires_independent_verification(self) -> bool {
        matches!(self, Severity::High | Severity::Critical)
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The three axes 13.24 says severity is assigned on.
///
/// Recorded alongside the judgement rather than combined into it, so a reader can see that a
/// "high" was called on result integrity rather than on infrastructure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactAxes {
    pub infrastructure: bool,
    pub data: bool,
    pub result_integrity: bool,
}

impl ImpactAxes {
    pub fn any(&self) -> bool {
        self.infrastructure || self.data || self.result_integrity
    }
}

/// The vulnerability classes 13.24 scopes in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VulnerabilityClass {
    CodeVulnerability,
    SandboxBypass,
    EvaluatorBypass,
    PrivacyLeakage,
    BenchmarkExploit,
    HiddenTestExposure,
    ProvenanceFlaw,
    MaliciousArtifact,
    DependencyCompromise,
    /// 13.24's most self-aware inclusion: a security claim that is not true is itself a
    /// vulnerability. This crate exists largely to make that class findable.
    MisleadingSecurityClaim,
}

impl VulnerabilityClass {
    pub fn as_str(self) -> &'static str {
        match self {
            VulnerabilityClass::CodeVulnerability => "code_vulnerability",
            VulnerabilityClass::SandboxBypass => "sandbox_bypass",
            VulnerabilityClass::EvaluatorBypass => "evaluator_bypass",
            VulnerabilityClass::PrivacyLeakage => "privacy_leakage",
            VulnerabilityClass::BenchmarkExploit => "benchmark_exploit",
            VulnerabilityClass::HiddenTestExposure => "hidden_test_exposure",
            VulnerabilityClass::ProvenanceFlaw => "provenance_flaw",
            VulnerabilityClass::MaliciousArtifact => "malicious_artifact",
            VulnerabilityClass::DependencyCompromise => "dependency_compromise",
            VulnerabilityClass::MisleadingSecurityClaim => "misleading_security_claim",
        }
    }
}

impl fmt::Display for VulnerabilityClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One stage transition, with the epoch it happened at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub to: Stage,
    pub epoch: u64,
    pub note: String,
}

impl Transition {
    pub fn to(stage: Stage, epoch: u64) -> Self {
        Transition {
            to: stage,
            epoch,
            note: String::new(),
        }
    }

    pub fn noting(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }
}

/// A tracked vulnerability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub class: VulnerabilityClass,
    pub severity: Severity,
    pub impact: ImpactAxes,
    pub stage: Stage,
    /// The epoch the current stage was entered at.
    pub entered_at: u64,
    /// Whether detail is under embargo. Advisory metadata may still show a generic quarantine.
    pub embargoed: bool,
    pub history: Vec<Transition>,
}

impl Vulnerability {
    pub fn reported(
        id: impl Into<String>,
        class: VulnerabilityClass,
        severity: Severity,
        epoch: u64,
    ) -> Self {
        Vulnerability {
            id: id.into(),
            class,
            severity,
            impact: ImpactAxes::default(),
            stage: Stage::Reported,
            entered_at: epoch,
            embargoed: true,
            history: Vec::new(),
        }
    }

    pub fn impacting(mut self, impact: ImpactAxes) -> Self {
        self.impact = impact;
        self
    }

    /// Moves one rung, or to a terminal outcome.
    pub fn advance(&mut self, transition: Transition) -> Result<(), SafetyError> {
        if self.stage.is_terminal() {
            return Err(SafetyError::LifecycleViolation {
                id: self.id.clone(),
                from: self.stage.to_string(),
                to: transition.to.to_string(),
                reason: format!("{} is terminal", self.stage),
            });
        }
        if transition.epoch < self.entered_at {
            return Err(SafetyError::EpochNotAdvancing {
                subject: self.id.clone(),
                previous: self.entered_at,
                epoch: transition.epoch,
            });
        }
        match (self.stage.rung(), transition.to.rung()) {
            (Some(from), Some(to)) if to == from + 1 => {}
            (Some(_), None) => {}
            (Some(from), Some(to)) if to <= from => {
                return Err(SafetyError::LifecycleViolation {
                    id: self.id.clone(),
                    from: self.stage.to_string(),
                    to: transition.to.to_string(),
                    reason: "the lifecycle does not run backwards".into(),
                });
            }
            _ => {
                return Err(SafetyError::LifecycleViolation {
                    id: self.id.clone(),
                    from: self.stage.to_string(),
                    to: transition.to.to_string(),
                    reason: "each stage must be entered in turn; a skipped stage is a stage \
                             nobody performed"
                        .into(),
                });
            }
        }
        self.entered_at = transition.epoch;
        self.stage = transition.to;
        if transition.to == Stage::Disclosed {
            self.embargoed = false;
        }
        self.history.push(transition);
        Ok(())
    }

    /// Epochs spent in the current stage, given the operator's current epoch.
    pub fn elapsed_in_stage(&self, now: u64) -> Option<u64> {
        now.checked_sub(self.entered_at)
    }
}

/// The advisory 13.24 requires at disclosure.
///
/// Every field is required. An advisory that omits result implications leaves a reader unable to
/// tell whether the numbers they are citing are still good, which for this platform is the whole
/// question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Advisory {
    pub affected_versions: String,
    pub impact: String,
    pub mitigation: String,
    pub fixed_versions: String,
    pub result_implications: String,
    pub timeline: String,
    pub credit: String,
    pub residual_risk: String,
}

impl Advisory {
    /// Field names that are empty.
    pub fn missing_fields(&self) -> Vec<&'static str> {
        let fields: [(&'static str, &str); 8] = [
            ("affected_versions", &self.affected_versions),
            ("impact", &self.impact),
            ("mitigation", &self.mitigation),
            ("fixed_versions", &self.fixed_versions),
            ("result_implications", &self.result_implications),
            ("timeline", &self.timeline),
            ("credit", &self.credit),
            ("residual_risk", &self.residual_risk),
        ];
        fields
            .into_iter()
            .filter(|(_, value)| value.trim().is_empty())
            .map(|(name, _)| name)
            .collect()
    }

    /// Gate for the `fixed → disclosed` transition.
    pub fn audit_for(&self, vulnerability: &Vulnerability) -> Result<(), SafetyError> {
        let missing = self.missing_fields();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(SafetyError::LifecycleViolation {
                id: vulnerability.id.clone(),
                from: vulnerability.stage.to_string(),
                to: Stage::Disclosed.to_string(),
                reason: format!("the advisory omits {}", missing.join(", ")),
            })
        }
    }
}

/// Whether a red-team report has been stood up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    /// Somebody said it happened.
    Reported,
    /// Someone else made it happen again.
    Reproduced,
    /// Reproduced and accepted as a real class.
    Confirmed,
    /// Attempted and did not reproduce. Kept, not deleted: a report that failed to reproduce is
    /// information about the report, not an absence of information.
    NotReproduced,
    Duplicate,
}

impl FindingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            FindingStatus::Reported => "reported",
            FindingStatus::Reproduced => "reproduced",
            FindingStatus::Confirmed => "confirmed",
            FindingStatus::NotReproduced => "not_reproduced",
            FindingStatus::Duplicate => "duplicate",
        }
    }
}

impl fmt::Display for FindingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One red-team result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    /// The campaign theme (13.21 runs them per boundary).
    pub campaign: String,
    /// The trust boundary attacked, as a [`crate::boundary::TrustZone`] name or a boundary label.
    pub boundary: String,
    pub status: FindingStatus,
    pub class: VulnerabilityClass,
    /// The reproduction, withheld while embargoed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduction: Option<String>,
}

impl Finding {
    pub fn new(
        id: impl Into<String>,
        campaign: impl Into<String>,
        boundary: impl Into<String>,
        class: VulnerabilityClass,
    ) -> Self {
        Finding {
            id: id.into(),
            campaign: campaign.into(),
            boundary: boundary.into(),
            status: FindingStatus::Reported,
            class,
            reproduction: None,
        }
    }

    pub fn with_status(mut self, status: FindingStatus) -> Self {
        self.status = status;
        self
    }

    pub fn reproducing(mut self, reproduction: impl Into<String>) -> Self {
        self.reproduction = Some(reproduction.into());
        self
    }

    /// 13.21's conversion. Confirmed findings only.
    pub fn into_regression_cell(self, embargoed: bool) -> Result<RegressionCell, SafetyError> {
        if self.status != FindingStatus::Confirmed {
            return Err(SafetyError::UnconfirmedFinding {
                finding: self.id.clone(),
                status: self.status.to_string(),
            });
        }
        Ok(RegressionCell {
            finding: self.id,
            campaign: self.campaign,
            boundary: self.boundary,
            class: self.class,
            minimised: false,
            embargoed,
            reproduction: self.reproduction,
        })
    }
}

/// A confirmed finding installed as a permanent sentinel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegressionCell {
    pub finding: String,
    pub campaign: String,
    pub boundary: String,
    pub class: VulnerabilityClass,
    /// 13.21 asks for a *minimized* microbenchmark. Recorded, because an unminimised cell is still
    /// a sentinel and pretending otherwise would delete it.
    pub minimised: bool,
    pub embargoed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reproduction: Option<String>,
}

impl RegressionCell {
    pub fn minimised(mut self) -> Self {
        self.minimised = true;
        self
    }

    /// What a public listing may show: the class and the boundary, never the reproduction.
    pub fn public_summary(&self) -> String {
        format!(
            "{} against {} ({}){}",
            self.finding,
            self.boundary,
            self.class,
            if self.embargoed {
                " — detail embargoed"
            } else {
                ""
            }
        )
    }
}

/// The registry of sentinels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Corpus {
    pub cells: Vec<RegressionCell>,
}

impl Corpus {
    pub fn push(&mut self, cell: RegressionCell) {
        self.cells.push(cell);
    }

    /// Every cell, embargoed or not. Embargo hides detail, not existence.
    pub fn sentinel_count(&self) -> usize {
        self.cells.len()
    }

    pub fn unminimised(&self) -> Vec<&RegressionCell> {
        self.cells.iter().filter(|cell| !cell.minimised).collect()
    }

    /// Boundaries with at least one sentinel.
    pub fn covered_boundaries(&self) -> BTreeSet<&str> {
        self.cells.iter().map(|cell| cell.boundary.as_str()).collect()
    }

    /// Boundaries in a supplied universe that no sentinel covers.
    pub fn uncovered(&self, universe: &[&str]) -> Vec<String> {
        let covered = self.covered_boundaries();
        universe
            .iter()
            .filter(|boundary| !covered.contains(*boundary))
            .map(|boundary| (*boundary).to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_advisory() -> Advisory {
        Advisory {
            affected_versions: "0.1.0".into(),
            impact: "hidden holdout readable".into(),
            mitigation: "rotate holdout".into(),
            fixed_versions: "0.1.1".into(),
            result_implications: "runs before r-40 are invalidated".into(),
            timeline: "e1 reported, e3 fixed".into(),
            credit: "reporter".into(),
            residual_risk: "older mirrors may retain the asset".into(),
        }
    }

    #[test]
    fn a_vulnerability_cannot_be_disclosed_before_it_is_fixed() {
        let mut vulnerability = Vulnerability::reported(
            "V-1",
            VulnerabilityClass::HiddenTestExposure,
            Severity::High,
            1,
        );
        vulnerability
            .advance(Transition::to(Stage::Triaged, 2))
            .expect("triage is the next rung");
        let error = vulnerability
            .advance(Transition::to(Stage::Disclosed, 3))
            .expect_err("publishing a live vulnerability is the thing this prevents");
        assert!(matches!(error, SafetyError::LifecycleViolation { .. }));
        assert_eq!(vulnerability.stage, Stage::Triaged);
    }

    #[test]
    fn the_lifecycle_does_not_run_backwards() {
        let mut vulnerability =
            Vulnerability::reported("V-2", VulnerabilityClass::SandboxBypass, Severity::Low, 1);
        vulnerability
            .advance(Transition::to(Stage::Triaged, 2))
            .expect("forward");
        let error = vulnerability
            .advance(Transition::to(Stage::Reported, 3))
            .expect_err("a triaged report does not become untriaged");
        assert!(error.to_string().contains("does not run backwards"), "{error}");
    }

    #[test]
    fn an_epoch_that_moves_backwards_is_refused_even_on_a_legal_rung() {
        let mut vulnerability =
            Vulnerability::reported("V-3", VulnerabilityClass::CodeVulnerability, Severity::Low, 9);
        assert!(matches!(
            vulnerability
                .advance(Transition::to(Stage::Triaged, 8))
                .expect_err("epochs are the only ordering there is"),
            SafetyError::EpochNotAdvancing { .. }
        ));
    }

    #[test]
    fn a_report_may_be_withdrawn_from_any_stage_before_disclosure() {
        let mut vulnerability =
            Vulnerability::reported("V-4", VulnerabilityClass::BenchmarkExploit, Severity::Low, 1);
        vulnerability
            .advance(Transition::to(Stage::Triaged, 2))
            .expect("forward");
        vulnerability
            .advance(Transition::to(Stage::Withdrawn, 3).noting("not a vulnerability"))
            .expect("withdrawal is always available before disclosure");
        assert_eq!(vulnerability.stage, Stage::Withdrawn);
    }

    #[test]
    fn a_terminal_vulnerability_accepts_no_further_transitions() {
        let mut vulnerability =
            Vulnerability::reported("V-5", VulnerabilityClass::PrivacyLeakage, Severity::Low, 1);
        vulnerability
            .advance(Transition::to(Stage::Duplicate, 2))
            .expect("terminal");
        assert!(vulnerability
            .advance(Transition::to(Stage::Triaged, 3))
            .is_err());
    }

    #[test]
    fn disclosure_clears_the_embargo_and_the_full_ladder_is_walkable() {
        let mut vulnerability = Vulnerability::reported(
            "V-6",
            VulnerabilityClass::EvaluatorBypass,
            Severity::Critical,
            1,
        );
        assert!(vulnerability.embargoed);
        for (stage, epoch) in [(Stage::Triaged, 2), (Stage::Fixed, 4), (Stage::Disclosed, 7)] {
            vulnerability
                .advance(Transition::to(stage, epoch))
                .expect("each rung in turn");
        }
        assert!(!vulnerability.embargoed);
        assert_eq!(vulnerability.history.len(), 3);
        assert_eq!(vulnerability.elapsed_in_stage(9), Some(2));
    }

    #[test]
    fn an_advisory_missing_result_implications_blocks_disclosure_and_names_the_field() {
        let vulnerability = Vulnerability::reported(
            "V-7",
            VulnerabilityClass::BenchmarkExploit,
            Severity::High,
            1,
        );
        let mut advisory = complete_advisory();
        advisory.result_implications = "  ".into();
        let error = advisory
            .audit_for(&vulnerability)
            .expect_err("a reader must be able to tell whether their numbers survived");
        assert!(error.to_string().contains("result_implications"), "{error}");
        assert!(complete_advisory().audit_for(&vulnerability).is_ok());
    }

    #[test]
    fn a_finding_nobody_reproduced_cannot_become_a_regression_cell() {
        let finding = Finding::new(
            "F-1",
            "hidden-test-extraction",
            "evaluator_sandbox",
            VulnerabilityClass::HiddenTestExposure,
        );
        let error = finding
            .into_regression_cell(true)
            .expect_err("a sentinel for an unconfirmed report asserts nothing");
        assert!(matches!(error, SafetyError::UnconfirmedFinding { .. }));
    }

    #[test]
    fn a_finding_that_failed_to_reproduce_is_kept_and_still_refused_as_a_cell() {
        let finding = Finding::new(
            "F-2",
            "browser-agents",
            "agent_sandbox",
            VulnerabilityClass::SandboxBypass,
        )
        .with_status(FindingStatus::NotReproduced);
        assert!(finding.clone().into_regression_cell(false).is_err());
        assert_eq!(finding.status, FindingStatus::NotReproduced);
    }

    #[test]
    fn a_confirmed_finding_becomes_a_sentinel_that_is_counted_even_while_embargoed() {
        let cell = Finding::new(
            "F-3",
            "hostile-supply-chain",
            "build_service",
            VulnerabilityClass::DependencyCompromise,
        )
        .with_status(FindingStatus::Confirmed)
        .reproducing("swap the base image digest after the manifest is signed")
        .into_regression_cell(true)
        .expect("confirmed");
        let mut corpus = Corpus::default();
        corpus.push(cell);
        assert_eq!(corpus.sentinel_count(), 1);
        let summary = corpus.cells[0].public_summary();
        assert!(summary.contains("detail embargoed"), "{summary}");
        assert!(!summary.contains("base image digest"), "{summary}");
    }

    #[test]
    fn an_unminimised_sentinel_is_listed_rather_than_silently_accepted() {
        let cell = Finding::new(
            "F-4",
            "federation",
            "private_worker",
            VulnerabilityClass::PrivacyLeakage,
        )
        .with_status(FindingStatus::Confirmed)
        .into_regression_cell(false)
        .expect("confirmed");
        let mut corpus = Corpus::default();
        corpus.push(cell.clone());
        assert_eq!(corpus.unminimised().len(), 1);
        let mut minimised = Corpus::default();
        minimised.push(cell.minimised());
        assert!(minimised.unminimised().is_empty());
    }

    #[test]
    fn boundaries_with_no_sentinel_are_reported_as_uncovered() {
        let mut corpus = Corpus::default();
        corpus.push(
            Finding::new(
                "F-5",
                "c",
                "agent_sandbox",
                VulnerabilityClass::SandboxBypass,
            )
            .with_status(FindingStatus::Confirmed)
            .into_regression_cell(false)
            .expect("confirmed"),
        );
        assert_eq!(
            corpus.uncovered(&["agent_sandbox", "evaluator_sandbox", "build_service"]),
            vec![
                "evaluator_sandbox".to_string(),
                "build_service".to_string()
            ]
        );
    }

    #[test]
    fn high_severity_findings_require_independent_verification_and_low_ones_do_not() {
        assert!(Severity::Critical.requires_independent_verification());
        assert!(Severity::High.requires_independent_verification());
        assert!(!Severity::Medium.requires_independent_verification());
    }

    #[test]
    fn a_misleading_security_claim_is_itself_a_tracked_vulnerability_class() {
        let vulnerability = Vulnerability::reported(
            "V-8",
            VulnerabilityClass::MisleadingSecurityClaim,
            Severity::Medium,
            1,
        )
        .impacting(ImpactAxes {
            result_integrity: true,
            ..ImpactAxes::default()
        });
        assert!(vulnerability.impact.any());
        assert_eq!(
            vulnerability.class.as_str(),
            "misleading_security_claim"
        );
    }
}
