//! Provider capability cards: a claim you cannot make until the test passes.
//!
//! Implements blueprint 05.12 (Runtime Conformance and Microbenchmarks). Its purpose line is the
//! argument for the module: "Test execution backends independently so benchmark results are not
//! confounded by hidden runtime defects."
//!
//! # Claiming is a function of evidence, not a field
//!
//! 05.12's release gate: "A provider cannot claim a capability until the corresponding conformance
//! tests pass on a public or reproducible environment."
//!
//! [`CapabilityCard`] has no setter for a claim. [`CapabilityCard::claims`] is derived from the
//! recorded [`ClaimState`]s and returns only the capabilities whose state is
//! [`ClaimState::Passed`]. [`CapabilityCard::claim`] — the accessor a caller reaches for when it
//! wants to *use* a capability — errors with [`SweepError::Unproven`] naming the actual state,
//! whether that state is `Untested` or `Failed`. This is the same shape as
//! `bioprism-registry`'s trust tiers, applied to runtimes rather than to packs.
//!
//! # Untested is not failed, and a differential over an untested capability is not agreement
//!
//! 05.12's cross-provider differential asks us to "run logically identical tasks and compare state
//! transitions and evaluator outcomes. Differences become provider notes or conformance failures."
//!
//! The trap is that a capability neither provider has tested compares equal. Two `Untested` states
//! are identical values, so a naive comparison reports agreement — and agreement is exactly what a
//! differential is looking for. [`differential`] therefore emits [`Drift::Indeterminate`] whenever
//! either side is untested, and only [`Drift::Agree`] when both sides have evidence that agrees.
//! An `Indeterminate` is not a difference and it is not agreement; it is the state the workspace
//! refuses to let collapse.
//!
//! # What is not implemented
//!
//! No tests are run. The three suites of 05.12 — correctness (file writes and rollback, background
//! process survival, service snapshots, clock control, seeded faults, secret isolation,
//! cancellation, timeout, artifact closure, branch independence), security (host escape, credential
//! exfiltration, network bypass, cross-trial contamination, malicious images, decompression bombs,
//! symlink and mount attacks) and performance (cold and warm startup, image pull, snapshot, resume,
//! fork, event throughput, artifact upload, cleanup, cache hit, parallel scaling) — are enumerated
//! as [`Check`] variants and executed nowhere. This crate has no sandbox, no container and no
//! clock.
//!
//! **No microbenchmark numbers.** 05.12's performance suite is a list of things to measure, and the
//! blueprint supplies no thresholds. [`Measurement`] therefore carries a caller-supplied value and
//! a caller-supplied unit and this module never compares one to a constant, because there is no
//! constant to compare it to and inventing one would put a number in the record that the blueprint
//! never authorised.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{require_nonempty, SweepError};

/// Which of 05.12's three suites a check belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Suite {
    Correctness,
    Security,
    Performance,
}

/// The conformance checks 05.12 enumerates, by suite.
///
/// The performance entries are here for completeness of the enumeration; they produce a
/// [`Measurement`], never a pass or a failure, because nothing in the blueprint says what value
/// would constitute passing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    FileWriteAndRollback,
    BackgroundProcessSurvival,
    ServiceSnapshot,
    ClockControl,
    SeededFaults,
    SecretIsolation,
    Cancellation,
    Timeout,
    ArtifactClosure,
    BranchIndependence,

    HostEscape,
    CredentialExfiltration,
    NetworkBypass,
    CrossTrialContamination,
    MaliciousImage,
    DecompressionBomb,
    SymlinkAndMountAttack,

    ColdStartup,
    WarmStartup,
    ImagePull,
    Snapshot,
    Resume,
    Fork,
    EventThroughput,
    ArtifactUpload,
    Cleanup,
    CacheHit,
    ParallelScaling,
}

impl Check {
    pub fn suite(self) -> Suite {
        match self {
            Check::FileWriteAndRollback
            | Check::BackgroundProcessSurvival
            | Check::ServiceSnapshot
            | Check::ClockControl
            | Check::SeededFaults
            | Check::SecretIsolation
            | Check::Cancellation
            | Check::Timeout
            | Check::ArtifactClosure
            | Check::BranchIndependence => Suite::Correctness,
            Check::HostEscape
            | Check::CredentialExfiltration
            | Check::NetworkBypass
            | Check::CrossTrialContamination
            | Check::MaliciousImage
            | Check::DecompressionBomb
            | Check::SymlinkAndMountAttack => Suite::Security,
            _ => Suite::Performance,
        }
    }

    /// Whether this check can pass or fail at all.
    ///
    /// False for the performance suite: a fork latency is a number, and calling it a pass requires
    /// a threshold nobody supplied.
    pub fn is_pass_fail(self) -> bool {
        self.suite() != Suite::Performance
    }
}

/// Where a conformance run happened. 05.12 requires "a public or reproducible environment".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRef {
    pub run_id: String,
    /// A reference a third party can use to re-run it. Required.
    pub reproducible_environment: String,
}

impl RunRef {
    pub fn new(
        run_id: impl Into<String>,
        reproducible_environment: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let run_id = run_id.into();
        let reproducible_environment = reproducible_environment.into();
        require_nonempty(&run_id, "RunRef", "run_id")?;
        require_nonempty(
            &reproducible_environment,
            "RunRef",
            "reproducible_environment",
        )?;
        Ok(RunRef { run_id, reproducible_environment })
    }
}

/// What is known about one capability of one provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ClaimState {
    /// Nobody ran the check.
    Untested,
    /// The check ran and the provider failed it. The witness is what makes the failure actionable.
    Failed { witness: String, run: RunRef },
    /// The check ran and passed, in a re-runnable environment.
    Passed { run: RunRef },
}

impl ClaimState {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClaimState::Untested => "untested",
            ClaimState::Failed { .. } => "failed",
            ClaimState::Passed { .. } => "passed",
        }
    }
}

/// A performance observation. Value and unit are the caller's; nothing here interprets them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub check: Check,
    pub value: f64,
    pub unit: String,
    pub run: RunRef,
}

/// What a provider has demonstrated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityCard {
    provider: String,
    states: BTreeMap<String, ClaimState>,
    measurements: Vec<Measurement>,
}

impl CapabilityCard {
    /// A new card claims nothing. Every check is [`ClaimState::Untested`] until evidence arrives.
    pub fn new(provider: impl Into<String>) -> Result<Self, SweepError> {
        let provider = provider.into();
        require_nonempty(&provider, "CapabilityCard", "provider")?;
        Ok(CapabilityCard { provider, states: BTreeMap::new(), measurements: Vec::new() })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// Record a pass. Refused for performance checks, which have no pass.
    pub fn record_pass(&mut self, check: Check, run: RunRef) -> Result<(), SweepError> {
        if !check.is_pass_fail() {
            return Err(SweepError::malformed(
                "CapabilityCard::record_pass",
                format!("{check:?} is a measurement; the blueprint supplies no passing threshold"),
            ));
        }
        self.states.insert(format!("{check:?}"), ClaimState::Passed { run });
        Ok(())
    }

    pub fn record_failure(
        &mut self,
        check: Check,
        witness: impl Into<String>,
        run: RunRef,
    ) -> Result<(), SweepError> {
        let witness = witness.into();
        require_nonempty(&witness, "CapabilityCard::record_failure", "witness")?;
        self.states.insert(format!("{check:?}"), ClaimState::Failed { witness, run });
        Ok(())
    }

    pub fn record_measurement(&mut self, measurement: Measurement) -> Result<(), SweepError> {
        if measurement.check.is_pass_fail() {
            return Err(SweepError::malformed(
                "CapabilityCard::record_measurement",
                format!("{:?} is a pass/fail check, not a measurement", measurement.check),
            ));
        }
        self.measurements.push(measurement);
        Ok(())
    }

    pub fn state(&self, check: Check) -> &ClaimState {
        self.states.get(&format!("{check:?}")).unwrap_or(&ClaimState::Untested)
    }

    pub fn measurements(&self) -> &[Measurement] {
        &self.measurements
    }

    /// Every capability this provider may say it has. Derived, never set.
    pub fn claims(&self) -> Vec<Check> {
        let mut claimed: Vec<Check> = Vec::new();
        for check in ALL_CHECKS {
            if matches!(self.state(check), ClaimState::Passed { .. }) {
                claimed.push(check);
            }
        }
        claimed
    }

    /// Use a capability, or find out why you may not.
    pub fn claim(&self, check: Check) -> Result<&RunRef, SweepError> {
        match self.state(check) {
            ClaimState::Passed { run } => Ok(run),
            other => Err(SweepError::Unproven {
                subject: format!("{}/{check:?}", self.provider),
                claim: "supported".to_string(),
                state: other.as_str(),
            }),
        }
    }
}

/// The pass/fail checks, in declaration order. Performance checks are excluded because they never
/// enter a claim.
pub const ALL_CHECKS: [Check; 17] = [
    Check::FileWriteAndRollback,
    Check::BackgroundProcessSurvival,
    Check::ServiceSnapshot,
    Check::ClockControl,
    Check::SeededFaults,
    Check::SecretIsolation,
    Check::Cancellation,
    Check::Timeout,
    Check::ArtifactClosure,
    Check::BranchIndependence,
    Check::HostEscape,
    Check::CredentialExfiltration,
    Check::NetworkBypass,
    Check::CrossTrialContamination,
    Check::MaliciousImage,
    Check::DecompressionBomb,
    Check::SymlinkAndMountAttack,
];

/// What comparing two providers on one check established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "drift")]
pub enum Drift {
    /// Both sides tested, same outcome.
    Agree,
    /// Both sides tested, different outcomes. 05.12's "provider note or conformance failure".
    Differ { left: String, right: String },
    /// At least one side never ran the check. Not agreement.
    Indeterminate { untested: Vec<String> },
}

/// Compare two providers check by check.
///
/// Only checks in [`ALL_CHECKS`] are compared. Two untested sides produce `Indeterminate`, which is
/// the whole reason this is a function rather than a `==`.
pub fn differential(left: &CapabilityCard, right: &CapabilityCard) -> BTreeMap<String, Drift> {
    let mut drifts = BTreeMap::new();
    for check in ALL_CHECKS {
        let (l, r) = (left.state(check), right.state(check));
        let mut untested = Vec::new();
        if matches!(l, ClaimState::Untested) {
            untested.push(left.provider.clone());
        }
        if matches!(r, ClaimState::Untested) {
            untested.push(right.provider.clone());
        }
        let drift = if !untested.is_empty() {
            Drift::Indeterminate { untested }
        } else if l.as_str() == r.as_str() {
            Drift::Agree
        } else {
            Drift::Differ { left: l.as_str().to_string(), right: r.as_str().to_string() }
        };
        drifts.insert(format!("{check:?}"), drift);
    }
    drifts
}

/// What a release gate decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum GateOutcome {
    Cleared,
    /// Named because a gate that says "no" without saying which check is a gate nobody can pass.
    Blocked { unproven: Vec<String> },
}

/// Gate a provider against the capabilities a deployment requires.
///
/// A required capability that is `Untested` blocks exactly as a `Failed` one does. That is the
/// gate's whole content: 05.12 does not say "a provider cannot claim a capability it failed", it
/// says it cannot claim one until the test *passes*.
pub fn gate(card: &CapabilityCard, required: &[Check]) -> GateOutcome {
    let unproven: Vec<String> = required
        .iter()
        .filter(|check| !matches!(card.state(**check), ClaimState::Passed { .. }))
        .map(|check| format!("{check:?}={}", card.state(*check).as_str()))
        .collect();
    if unproven.is_empty() {
        GateOutcome::Cleared
    } else {
        GateOutcome::Blocked { unproven }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> RunRef {
        RunRef::new("run-1", "ghcr.io/example/conformance@sha256:abc").unwrap()
    }

    fn card(provider: &str) -> CapabilityCard {
        CapabilityCard::new(provider).unwrap()
    }

    #[test]
    fn a_new_card_claims_nothing() {
        let c = card("firecracker");
        assert!(c.claims().is_empty());
        assert_eq!(c.state(Check::HostEscape), &ClaimState::Untested);
        assert_eq!(c.provider(), "firecracker");
    }

    #[test]
    fn a_capability_cannot_be_used_until_its_check_passes() {
        let mut c = card("threads");
        let err = c.claim(Check::SecretIsolation).unwrap_err();
        assert!(matches!(err, SweepError::Unproven { state: "untested", .. }));
        c.record_pass(Check::SecretIsolation, run()).unwrap();
        assert_eq!(c.claim(Check::SecretIsolation).unwrap().run_id, "run-1");
    }

    #[test]
    fn a_failed_check_is_reported_as_failed_rather_than_as_untested() {
        let mut c = card("threads");
        c.record_failure(Check::HostEscape, "escaped via /proc/self/root", run()).unwrap();
        let err = c.claim(Check::HostEscape).unwrap_err();
        assert!(matches!(err, SweepError::Unproven { state: "failed", .. }));
        assert!(c.claims().is_empty());
    }

    #[test]
    fn a_failure_without_a_witness_is_refused() {
        let mut c = card("threads");
        assert!(c.record_failure(Check::HostEscape, "  ", run()).is_err());
    }

    #[test]
    fn a_conformance_run_must_name_a_reproducible_environment() {
        assert!(RunRef::new("run-1", "").is_err());
        assert!(RunRef::new("", "env").is_err());
    }

    #[test]
    fn a_performance_check_cannot_be_recorded_as_a_pass_because_no_threshold_exists() {
        let mut c = card("threads");
        let err = c.record_pass(Check::ColdStartup, run()).unwrap_err();
        assert!(matches!(err, SweepError::Malformed { .. }));
        assert!(!Check::ColdStartup.is_pass_fail());
        assert!(Check::Timeout.is_pass_fail());
    }

    #[test]
    fn a_pass_fail_check_cannot_be_smuggled_in_as_a_measurement() {
        let mut c = card("threads");
        let err = c
            .record_measurement(Measurement {
                check: Check::Timeout,
                value: 1.0,
                unit: "s".into(),
                run: run(),
            })
            .unwrap_err();
        assert!(matches!(err, SweepError::Malformed { .. }));
    }

    #[test]
    fn a_measurement_is_stored_without_being_compared_to_anything() {
        let mut c = card("threads");
        c.record_measurement(Measurement {
            check: Check::Fork,
            value: 12.5,
            unit: "milliseconds".into(),
            run: run(),
        })
        .unwrap();
        assert_eq!(c.measurements().len(), 1);
        assert!(c.claims().is_empty());
    }

    #[test]
    fn the_suites_partition_the_checks_and_only_two_of_them_are_claimable() {
        assert_eq!(ALL_CHECKS.len(), 17);
        assert!(ALL_CHECKS.iter().all(|c| c.is_pass_fail()));
        assert_eq!(Check::HostEscape.suite(), Suite::Security);
        assert_eq!(Check::ClockControl.suite(), Suite::Correctness);
        assert_eq!(Check::CacheHit.suite(), Suite::Performance);
    }

    #[test]
    fn two_untested_providers_do_not_agree_they_are_indeterminate() {
        let drifts = differential(&card("a"), &card("b"));
        match &drifts["HostEscape"] {
            Drift::Indeterminate { untested } => assert_eq!(untested, &["a", "b"]),
            other => panic!("expected Indeterminate, got {other:?}"),
        }
    }

    #[test]
    fn one_tested_side_against_one_untested_side_is_still_indeterminate() {
        let mut a = card("a");
        a.record_pass(Check::Cancellation, run()).unwrap();
        let drifts = differential(&a, &card("b"));
        match &drifts["Cancellation"] {
            Drift::Indeterminate { untested } => assert_eq!(untested, &["b"]),
            other => panic!("expected Indeterminate, got {other:?}"),
        }
    }

    #[test]
    fn two_tested_providers_with_the_same_outcome_agree() {
        let mut a = card("a");
        let mut b = card("b");
        a.record_pass(Check::Timeout, run()).unwrap();
        b.record_pass(Check::Timeout, run()).unwrap();
        assert_eq!(differential(&a, &b)["Timeout"], Drift::Agree);
    }

    #[test]
    fn two_tested_providers_with_different_outcomes_drift() {
        let mut a = card("a");
        let mut b = card("b");
        a.record_pass(Check::NetworkBypass, run()).unwrap();
        b.record_failure(Check::NetworkBypass, "egress reached 1.1.1.1", run()).unwrap();
        match &differential(&a, &b)["NetworkBypass"] {
            Drift::Differ { left, right } => {
                assert_eq!(left, "passed");
                assert_eq!(right, "failed");
            }
            other => panic!("expected Differ, got {other:?}"),
        }
    }

    #[test]
    fn the_release_gate_blocks_on_untested_exactly_as_it_blocks_on_failed() {
        let mut c = card("threads");
        c.record_failure(Check::HostEscape, "escaped", run()).unwrap();
        match gate(&c, &[Check::HostEscape, Check::SecretIsolation]) {
            GateOutcome::Blocked { unproven } => {
                assert!(unproven.contains(&"HostEscape=failed".to_string()));
                assert!(unproven.contains(&"SecretIsolation=untested".to_string()));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn the_release_gate_clears_when_every_required_check_passed() {
        let mut c = card("threads");
        c.record_pass(Check::HostEscape, run()).unwrap();
        c.record_pass(Check::SecretIsolation, run()).unwrap();
        assert_eq!(
            gate(&c, &[Check::HostEscape, Check::SecretIsolation]),
            GateOutcome::Cleared
        );
        assert_eq!(c.claims().len(), 2);
    }

    #[test]
    fn a_provider_name_cannot_be_empty() {
        assert!(CapabilityCard::new("   ").is_err());
    }
}
