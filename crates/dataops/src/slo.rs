//! Observability and service-level objectives (12.12): failure attribution, error budgets, and
//! the arithmetic of not knowing.
//!
//! 12.12 gives an eight-class failure taxonomy, says "classification evidence and confidence are
//! retained", lists eight initial SLOs, ends with "Evaluation success is not a service SLO", and
//! asks for burn-rate alerts. Everything in that list is a predicate over a set of observations,
//! which makes this the most directly implementable of the seven modules — and the one where the
//! section's silences do the most damage, because SLO arithmetic looks the same whether or not
//! the numbers underneath it mean anything.
//!
//! # Three silences, and what is done about each
//!
//! **No denominator.** "API read availability" and "operation acceptance" are named as objectives
//! with no statement of what population they are measured over. An implementation that divides
//! the successes it saw by the requests it received telemetry for produces a number, and that
//! number improves when telemetry breaks. [`Coverage::NoDenominator`] is what
//! [`ServiceObjective::evaluate`] receives in that case, and it returns
//! [`Conformance::Indeterminate`] rather than a rate.
//!
//! **No statement of which failures charge the budget.** The taxonomy separates platform
//! infrastructure from provider from agent, and the section says only that evaluation success is
//! not a service SLO. Which of the eight consume the error budget is left open, so it is a
//! [`BudgetPolicy`] the caller supplies rather than a constant baked in here.
//! [`BudgetPolicy::platform_default`] is a stated reading of the section, not the section.
//!
//! **No rule for an unattributed failure.** The section requires evidence and confidence to be
//! retained and never says what happens when neither exists. This module fails closed:
//! [`Attribution::Unclassified`] charges the budget. The consequence is the invariant worth
//! stating out loud — **an objective cannot be improved by failing to classify a failure** — and
//! the alternative, excluding what you could not explain, is an availability figure that rises as
//! the diagnosis gets worse.
//!
//! # Partial coverage proves breaches and never proves attainment
//!
//! The asymmetry is the useful part. Unobserved requests can only add failures, so if the
//! failures already counted exceed what the objective permits over the *whole* declared
//! population, the breach is certain however incomplete the telemetry is. The converse never
//! holds: an attainment computed over a subset says nothing about the rest.
//! [`ServiceObjective::evaluate`] implements exactly that, which is why partial coverage is not
//! simply refused.
//!
//! # Exact arithmetic, no floats
//!
//! A target is a rational — `999 per 1000`, not `0.999` — and every comparison is integer
//! multiplication in `u128`. There is no rounding anywhere in this module, so an objective at
//! exactly its limit is decided the same way on every machine, and a report can be hashed.
//!
//! # Not implemented
//!
//! No telemetry collection, no metrics, no traces, no log pipeline, no alert delivery, no
//! dashboards. [`Observations`] is a struct the caller fills in from a system this crate cannot
//! see. Correlated trace ids are named in 12.12 and are not modelled; `bioprism-ledger` owns the
//! event record and this module does not emit one. There is no time: a window is a pair of
//! caller-supplied epochs, so "burn rate per hour" is not expressible and the burn rate here is
//! per window, which is a weaker thing and is labelled as such.

use crate::basis::{Attested, Basis, Coverage};
use crate::error::{check_name, SloError};
use bioprism_infra::Epoch;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// 12.12's failure taxonomy, verbatim and closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureDomain {
    PlatformInfrastructure,
    Provider,
    BenchmarkEnvironment,
    Adapter,
    Agent,
    Evaluator,
    Policy,
    UserCancellation,
}

impl FailureDomain {
    pub const ALL: [FailureDomain; 8] = [
        FailureDomain::PlatformInfrastructure,
        FailureDomain::Provider,
        FailureDomain::BenchmarkEnvironment,
        FailureDomain::Adapter,
        FailureDomain::Agent,
        FailureDomain::Evaluator,
        FailureDomain::Policy,
        FailureDomain::UserCancellation,
    ];

    pub fn name(self) -> &'static str {
        match self {
            FailureDomain::PlatformInfrastructure => "platform-infrastructure",
            FailureDomain::Provider => "provider",
            FailureDomain::BenchmarkEnvironment => "benchmark-environment",
            FailureDomain::Adapter => "adapter",
            FailureDomain::Agent => "agent",
            FailureDomain::Evaluator => "evaluator",
            FailureDomain::Policy => "policy",
            FailureDomain::UserCancellation => "user-cancellation",
        }
    }

    /// Whether the outcome belongs to the evaluation rather than to the service.
    ///
    /// This is 12.12's "evaluation success is not a service SLO" as a predicate. It is *not* the
    /// same question as whether the domain charges the budget — that is [`BudgetPolicy`]'s, and
    /// keeping them apart matters because a benchmark environment failure is an evaluation
    /// outcome that a platform may still choose to charge itself for.
    pub fn is_evaluation_outcome(self) -> bool {
        matches!(
            self,
            FailureDomain::Agent | FailureDomain::Evaluator | FailureDomain::BenchmarkEnvironment
        )
    }
}

impl fmt::Display for FailureDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How sure the classifier was.
///
/// Retained rather than thresholded. 12.12 asks for confidence to be kept, and a classifier that
/// dropped everything below a bar would silently convert weak attributions into unclassified
/// ones, which changes the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Certain,
    Probable,
    Weak,
}

/// A failure and what is known about whose fault it was.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "attribution", rename_all = "snake_case")]
pub enum Attribution {
    Classified {
        domain: FailureDomain,
        evidence: Vec<String>,
        confidence: Confidence,
    },
    /// No domain could be determined. Charges the budget.
    Unclassified { reason: String },
}

impl Attribution {
    /// Classifies a failure. Requires at least one piece of evidence.
    ///
    /// 12.12 says classification evidence is retained; a constructor that accepted an empty
    /// evidence list would let a caller assert a domain from nothing and have it counted the same
    /// as one derived from a stack trace. The refusal is the enforcement.
    pub fn classified(
        domain: FailureDomain,
        evidence: impl IntoIterator<Item = String>,
        confidence: Confidence,
    ) -> Result<Self, SloError> {
        let evidence: Vec<String> = evidence.into_iter().collect();
        if evidence.is_empty() {
            return Err(SloError::MalformedField {
                field: "classification evidence",
                value: String::new(),
            });
        }
        Ok(Attribution::Classified {
            domain,
            evidence,
            confidence,
        })
    }

    pub fn unclassified(reason: impl Into<String>) -> Self {
        Attribution::Unclassified {
            reason: reason.into(),
        }
    }

    pub fn domain(&self) -> Option<FailureDomain> {
        match self {
            Attribution::Classified { domain, .. } => Some(*domain),
            Attribution::Unclassified { .. } => None,
        }
    }
}

/// Which failure domains consume a service error budget.
///
/// A value rather than a constant because 12.12 does not say. Anything not listed is excluded,
/// and an [`Attribution::Unclassified`] is charged regardless of the set — there is no
/// constructor that turns that off.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetPolicy {
    chargeable: BTreeSet<FailureDomain>,
}

impl BudgetPolicy {
    /// A stated reading of 12.12, not a quotation of it.
    ///
    /// Charges platform infrastructure, adapter and provider. Provider is the contestable one:
    /// the section lists it separately from platform infrastructure, which suggests it is somebody
    /// else's fault, but a user whose run did not start cannot tell the difference and the SLO is
    /// a promise to that user. Excluding it would let a platform hit its availability target by
    /// choosing unreliable providers.
    pub fn platform_default() -> Self {
        BudgetPolicy {
            chargeable: [
                FailureDomain::PlatformInfrastructure,
                FailureDomain::Adapter,
                FailureDomain::Provider,
            ]
            .into_iter()
            .collect(),
        }
    }

    pub fn charging(domains: impl IntoIterator<Item = FailureDomain>) -> Self {
        BudgetPolicy {
            chargeable: domains.into_iter().collect(),
        }
    }

    /// Whether this failure consumes budget.
    ///
    /// Unclassified always does. That is the fail-closed direction, and it is the reason the
    /// method takes an [`Attribution`] rather than a [`FailureDomain`]: there is no way to ask
    /// the question about a failure whose domain is unknown and get "no".
    pub fn charges(&self, attribution: &Attribution) -> bool {
        match attribution {
            Attribution::Classified { domain, .. } => self.chargeable.contains(domain),
            Attribution::Unclassified { .. } => true,
        }
    }

    pub fn chargeable(&self) -> &BTreeSet<FailureDomain> {
        &self.chargeable
    }
}

/// A target expressed as an exact rational: `good` successes permitted per `per` attempts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Target {
    good: u64,
    per: u64,
}

impl Target {
    /// `999 per 1000` rather than `0.999`, so nothing rounds.
    pub fn new(good: u64, per: u64, objective: &str) -> Result<Self, SloError> {
        if per == 0 || good > per {
            return Err(SloError::ImpossibleTarget {
                name: objective.to_string(),
                good,
                total: per,
            });
        }
        Ok(Target { good, per })
    }

    pub fn good(self) -> u64 {
        self.good
    }

    pub fn per(self) -> u64 {
        self.per
    }

    /// How many failures a population of `population` is permitted, rounding down.
    ///
    /// Rounding down is the strict direction: a target of 999/1000 over 1500 requests permits one
    /// failure, not one and a half, and permitting two would make the objective weaker than
    /// stated for every population that is not a multiple of `per`.
    pub fn allowance(self, population: u64) -> u64 {
        let permitted = (population as u128) * ((self.per - self.good) as u128) / (self.per as u128);
        u64::try_from(permitted).unwrap_or(u64::MAX)
    }
}

/// The epoch range an objective is measured over.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Window {
    start: Epoch,
    end: Epoch,
}

impl Window {
    pub fn new(start: Epoch, end: Epoch) -> Result<Self, SloError> {
        if end < start {
            return Err(SloError::WindowInverted {
                start: start.tick(),
                end: end.tick(),
            });
        }
        Ok(Window { start, end })
    }

    pub fn start(self) -> Epoch {
        self.start
    }

    pub fn end(self) -> Epoch {
        self.end
    }

    pub fn spans(self) -> u64 {
        self.end.tick() - self.start.tick()
    }
}

/// What was seen in a window.
///
/// `coverage` is the population statement and is separate from the counts on purpose: `good` and
/// `failures` describe the events that arrived, `coverage` describes how many should have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observations {
    pub good: u64,
    pub failures: Vec<Attribution>,
    pub coverage: Coverage,
}

impl Observations {
    pub fn new(
        good: u64,
        failures: impl IntoIterator<Item = Attribution>,
        coverage: Coverage,
    ) -> Self {
        Observations {
            good,
            failures: failures.into_iter().collect(),
            coverage,
        }
    }

    pub fn seen(&self) -> u64 {
        self.good.saturating_add(self.failures.len() as u64)
    }
}

/// Why an objective could not be decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum Indeterminate {
    /// The population the observations came from is unknown, so no rate exists.
    NoDenominator { observed: u64, note: String },
    /// Part of the population was not observed, and the failures seen do not by themselves
    /// establish a breach.
    PartialCoverage { observed: u64, expected: u64 },
    /// Nothing at all was observed.
    NoObservations,
}

/// Whether an objective was met, and if it cannot be said, why not.
///
/// There is no `is_ok`. [`Conformance::is_met`] tests the one variant it names, and
/// [`Conformance::Indeterminate`] is not a synonym for either of the other two — the same
/// position `bioprism-hubapi` and `bioprism-tokens` take on their freshness types, applied to a
/// figure an operator will otherwise put on a dashboard next to two that mean something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "conformance", rename_all = "snake_case")]
pub enum Conformance {
    Met { charged: u64, allowance: u64 },
    Breached { charged: u64, allowance: u64 },
    Indeterminate(Indeterminate),
}

impl Conformance {
    pub fn is_met(&self) -> bool {
        matches!(self, Conformance::Met { .. })
    }

    pub fn is_breached(&self) -> bool {
        matches!(self, Conformance::Breached { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Conformance::Met { .. } => "met",
            Conformance::Breached { .. } => "breached",
            Conformance::Indeterminate(_) => "indeterminate",
        }
    }
}

/// The evaluation of one objective over one window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SloReport {
    pub objective: String,
    pub window: Window,
    pub conformance: Conformance,
    pub charged: u64,
    pub excluded: BTreeMap<FailureDomain, u64>,
    pub unclassified: u64,
    pub coverage: Coverage,
}

/// A named objective with a target and a window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceObjective {
    name: String,
    target: Target,
    window: Window,
}

impl ServiceObjective {
    pub fn new(name: impl Into<String>, target: Target, window: Window) -> Result<Self, SloError> {
        let name = name.into();
        if !check_name(&name) {
            return Err(SloError::MalformedField {
                field: "objective name",
                value: name,
            });
        }
        Ok(ServiceObjective {
            name,
            target,
            window,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn window(&self) -> Window {
        self.window
    }

    /// Decides the objective, or says why it cannot be decided.
    ///
    /// The order of the branches is the argument. A certain breach is reported first, because it
    /// is sound under any coverage; only then does completeness matter, and only for concluding
    /// that the objective *held*.
    pub fn evaluate(&self, observations: &Observations, policy: &BudgetPolicy) -> SloReport {
        let mut charged = 0u64;
        let mut unclassified = 0u64;
        let mut excluded: BTreeMap<FailureDomain, u64> = BTreeMap::new();
        for failure in &observations.failures {
            if policy.charges(failure) {
                charged = charged.saturating_add(1);
            } else if let Some(domain) = failure.domain() {
                *excluded.entry(domain).or_insert(0) += 1;
            }
            if failure.domain().is_none() {
                unclassified = unclassified.saturating_add(1);
            }
        }

        let conformance = match observations.coverage.expected() {
            None => Conformance::Indeterminate(Indeterminate::NoDenominator {
                observed: observations.coverage.observed(),
                note: "no population was declared, so no rate exists".to_string(),
            }),
            Some(0) => Conformance::Indeterminate(Indeterminate::NoObservations),
            Some(expected) => {
                let allowance = self.target.allowance(expected);
                if charged > allowance {
                    Conformance::Breached { charged, allowance }
                } else if observations.coverage.is_complete() {
                    Conformance::Met { charged, allowance }
                } else {
                    Conformance::Indeterminate(Indeterminate::PartialCoverage {
                        observed: observations.coverage.observed(),
                        expected,
                    })
                }
            }
        };

        SloReport {
            objective: self.name.clone(),
            window: self.window,
            conformance,
            charged,
            excluded,
            unclassified,
            coverage: observations.coverage.clone(),
        }
    }

    /// The report, wrapped in the basis the telemetry behind it had.
    ///
    /// Telemetry is never first-hand to this crate — it was collected by something else — so the
    /// caller states the basis and it travels with the number. A dashboard that renders
    /// [`Attested::value`] without rendering [`Attested::basis`] is the failure this makes
    /// visible rather than the one it prevents.
    pub fn evaluate_attested(
        &self,
        observations: &Observations,
        policy: &BudgetPolicy,
        basis: Basis,
    ) -> Attested<SloReport> {
        let report = self.evaluate(observations, policy);
        let coverage = report.coverage.clone();
        Attested::new(report, basis, coverage)
    }
}

/// A burn-rate threshold as an exact rational: fire above `times` per `per` of the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BurnThreshold {
    times: u64,
    per: u64,
}

impl BurnThreshold {
    pub fn new(times: u64, per: u64) -> Result<Self, SloError> {
        if per == 0 {
            return Err(SloError::ImpossibleTarget {
                name: "burn threshold".to_string(),
                good: times,
                total: per,
            });
        }
        Ok(BurnThreshold { times, per })
    }
}

/// What an alerting rule concluded.
///
/// Three states, and the third is the point. A window whose telemetry has a hole is not quiet;
/// it is unmeasured, and an operator seeing "no alert" has been told something false.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "alert", rename_all = "snake_case")]
pub enum AlertDecision {
    /// Budget is burning faster than the threshold.
    Fire { charged: u64, allowance: u64 },
    /// Budget is being consumed within the threshold.
    Quiet { charged: u64, allowance: u64 },
    /// The rule could not be evaluated on this window.
    CannotEvaluate { reason: Indeterminate },
}

impl AlertDecision {
    pub fn is_fire(&self) -> bool {
        matches!(self, AlertDecision::Fire { .. })
    }

    /// True only for [`AlertDecision::Quiet`].
    ///
    /// Deliberately not named `is_ok` and deliberately false for `CannotEvaluate`, so a caller
    /// writing `if !decision.is_fire()` and a caller writing `if decision.is_quiet()` get
    /// different behaviour on an unmeasured window — and the second one is right.
    pub fn is_quiet(&self) -> bool {
        matches!(self, AlertDecision::Quiet { .. })
    }
}

/// Applies a burn-rate threshold to a report.
///
/// `charged * threshold.per > allowance * threshold.times` in `u128`: exact, no division, no
/// rounding. An allowance of zero with any charged failure fires, which is correct — a budget of
/// nothing is burned by the first failure.
pub fn burn_rate_alert(report: &SloReport, threshold: BurnThreshold) -> AlertDecision {
    match &report.conformance {
        Conformance::Indeterminate(reason) => AlertDecision::CannotEvaluate {
            reason: reason.clone(),
        },
        Conformance::Met { charged, allowance } | Conformance::Breached { charged, allowance } => {
            let burned = (*charged as u128) * (threshold.per as u128);
            let permitted = (*allowance as u128) * (threshold.times as u128);
            if burned > permitted {
                AlertDecision::Fire {
                    charged: *charged,
                    allowance: *allowance,
                }
            } else {
                AlertDecision::Quiet {
                    charged: *charged,
                    allowance: *allowance,
                }
            }
        }
    }
}

/// The eight objectives 12.12 names, as a set of declared names with no targets.
///
/// The names are the section's; the targets are not, because it gives none. This function returns
/// what the section actually specifies — a list of things somebody must pick a number for — so
/// that the gap is a value in the code rather than a sentence in a comment.
pub fn declared_objective_names() -> [&'static str; 8] {
    [
        "api-read-availability",
        "operation-acceptance",
        "artifact-durability",
        "queue-start-latency-by-priority",
        "successful-resumability",
        "publication-atomicity",
        "registry-index-freshness",
        "security-event-response",
    ]
}
