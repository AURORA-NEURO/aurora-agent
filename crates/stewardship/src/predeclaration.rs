//! Pre-registration: a metric chosen before the result and one chosen after are different objects.
//!
//! Implements blueprint 14.10 (Metric, Score and Statistical Governance) — its metric registry, its
//! predeclaration paragraph, its clustering rule, and its six-way taxonomy of trials that did not
//! produce a score.
//!
//! # The move, and where it comes from
//!
//! `bioprism_lab::holdout` faced the neighbouring problem and solved it by making the honest object
//! the only representable one: `CleanMeasurement` has private fields, one constructor that checks
//! exposure first, and `Serialize` without `Deserialize`, so a score cannot enter a report without
//! the ledger that vouched for it. Contamination is an error, not a flag somebody might read.
//!
//! The metric-selection equivalent is this module. A holdout is burned by *selection*; an analysis
//! is burned by *choice of metric*, and the burn is just as invisible after the fact. Twelve
//! metrics were computed, one is reported, and the report is indistinguishable from one where a
//! single metric was declared in advance — unless the declaration exists as a citable object.
//!
//! So there are two finding types and no path between them:
//!
//! - [`ConfirmatoryFinding`] — private fields, one constructor ([`SealedPlan::confirm`]),
//!   `Serialize` but **not** `Deserialize`. It can only be obtained from a plan that was sealed
//!   before the results were observed and that names the metric being reported.
//! - [`ExploratoryFinding`] — fully serde, constructible from anything, and permanently labelled.
//!   There is no `promote`, no `into_confirmatory`, and no argument anywhere that upgrades one.
//!
//! An exploratory finding is not a lesser finding. It is a different claim, and 14.11 refuses to
//! let a research causal claim rest on one.
//!
//! # Exclusions are stated over dispositions, never over outcomes
//!
//! 14.10: *"Timeout, infrastructure error, policy refusal, invalid output, grader unknown, and
//! canceled trial are distinct. Rules cannot selectively drop poor outcomes."* The second sentence
//! is the one that is normally violated, and it is normally violated honestly — an analyst excludes
//! "runs that went wrong", and the runs that went wrong were disproportionately the failures.
//!
//! [`ExclusionRule`] therefore names a [`TrialDisposition`] and nothing else. There is no
//! `ExclusionRule::below(0.3)`, no predicate over a score, and no free-text criterion, so an
//! outcome-dependent exclusion has no spelling. The one disposition that cannot be excluded is
//! [`TrialDisposition::Scored`]: excluding trials that produced a score is selective reporting with
//! a rule attached, and it is [`PredeclarationError::ExclusionOfScoredTrials`] at construction.
//!
//! # Clustering
//!
//! 14.10: *"Generated siblings and repeated trials are not independent. Parent/cell-level
//! aggregation or hierarchical models are mandatory for uncertainty."* A plan states its
//! [`ClusteringUnit`] and the number of independent units it expects; results that report more
//! trials than units without naming the unit are refused. This crate computes no interval — that is
//! `bioprism-metrics`' — but it refuses to let a plan be silent about the thing an interval would
//! be wrong about.
//!
//! # Not implemented
//!
//! No estimators, no intervals, no multiple-comparison correction, no power analysis, no effect
//! size. `bioprism-metrics` owns the arithmetic above a measurement and `bioprism-atlas` owns the
//! measurement itself; this module governs the *plan*. No composite weighting either:
//! `bioprism_metrics::DeclaredWeighting` already carries a digest of the weighting policy and
//! refuses a default, and a second declared-weighting type in one workspace is how the two drift.
//! No clock — a plan is sealed at an operator epoch, and "before the results" means a smaller
//! integer.

use crate::error::PredeclarationError;
use crate::id::Epoch;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Which way is better. 14.10's metric registry lists "direction" as a required field, and a
/// registry that omits it produces leaderboards sorted the wrong way for half its metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricDirection {
    HigherIsBetter,
    LowerIsBetter,
}

/// The fields 14.10 requires of a metric registry entry, minus the ones nobody can check.
///
/// The module lists thirteen: "Name, version, construct, formula/code, unit, direction, valid
/// range, aggregation population, clustering, missingness, uncertainty, known failure modes, and
/// owner." `construct` here is the free-text statement of *what the number is supposed to mean*,
/// which is the field that matters and the field most often absent. `formula/code` is represented
/// by a digest rather than by code, because this crate does not execute metrics; two entries
/// claiming the same version with different digests is the redefinition 14.10 exists to catch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricDefinition {
    pub name: String,
    pub version: String,
    /// What the number is supposed to measure, in words. Refused when empty.
    pub construct: String,
    /// Digest of the formula or implementation. Identity of the computation, not the computation.
    pub definition_digest: ContentHash,
    pub direction: MetricDirection,
    pub owner: String,
    /// 14.10's "known failure modes" field. An empty list is permitted and is itself informative;
    /// unlike the oracle review in [`crate::review`], a metric with no known failure mode is a
    /// common and not necessarily dishonest state.
    #[serde(default)]
    pub known_failure_modes: Vec<String>,
    /// Whether this metric is a gate rather than a tradeable point. 14.10: "Safety-critical metrics
    /// are gates rather than tradeable points." The flag is recorded here and enforced by
    /// `bioprism_metrics::gate`, which owns release gates; this crate does not compute composites.
    #[serde(default)]
    pub safety_gate: bool,
}

impl MetricDefinition {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        construct: impl Into<String>,
        definition_digest: ContentHash,
        direction: MetricDirection,
        owner: impl Into<String>,
    ) -> Self {
        MetricDefinition {
            name: name.into(),
            version: version.into(),
            construct: construct.into(),
            definition_digest,
            direction,
            owner: owner.into(),
            known_failure_modes: Vec::new(),
            safety_gate: false,
        }
    }

    #[must_use]
    pub fn as_safety_gate(mut self) -> Self {
        self.safety_gate = true;
        self
    }

    /// Name and version together, which is what a plan cites.
    pub fn key(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// 14.10's metric registry.
///
/// The one rule it enforces is the one that makes a version number mean something: a
/// `name@version` may be registered once, and a second registration under the same key with a
/// different definition digest or construct is [`PredeclarationError::MetricRedefinedInPlace`]. A
/// registry that lets a definition drift under a fixed version is how "we always reported the same
/// metric" becomes true and meaningless at the same time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MetricRegistry {
    entries: BTreeMap<String, MetricDefinition>,
}

impl MetricRegistry {
    pub fn new() -> Self {
        MetricRegistry::default()
    }

    pub fn register(&mut self, definition: MetricDefinition) -> Result<(), PredeclarationError> {
        let key = definition.key();
        if let Some(existing) = self.entries.get(&key) {
            if existing.definition_digest != definition.definition_digest
                || existing.construct != definition.construct
                || existing.direction != definition.direction
            {
                return Err(PredeclarationError::MetricRedefinedInPlace {
                    metric: definition.name,
                    version: definition.version,
                });
            }
            return Ok(());
        }
        self.entries.insert(key, definition);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&MetricDefinition> {
        self.entries.get(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// What happened to a trial. 14.10's six non-scoring outcomes plus the one that scored.
///
/// Closed and flat: the module's point is that these are *distinct*, so a variant meaning "other"
/// would let every inconvenient case be filed as one thing and then excluded as one thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrialDisposition {
    /// The trial produced a score. This is the disposition that can never be excluded.
    Scored,
    Timeout,
    InfrastructureError,
    PolicyRefusal,
    InvalidOutput,
    GraderUnknown,
    Cancelled,
}

impl TrialDisposition {
    pub const ALL: [TrialDisposition; 7] = [
        TrialDisposition::Scored,
        TrialDisposition::Timeout,
        TrialDisposition::InfrastructureError,
        TrialDisposition::PolicyRefusal,
        TrialDisposition::InvalidOutput,
        TrialDisposition::GraderUnknown,
        TrialDisposition::Cancelled,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            TrialDisposition::Scored => "scored",
            TrialDisposition::Timeout => "timeout",
            TrialDisposition::InfrastructureError => "infrastructure_error",
            TrialDisposition::PolicyRefusal => "policy_refusal",
            TrialDisposition::InvalidOutput => "invalid_output",
            TrialDisposition::GraderUnknown => "grader_unknown",
            TrialDisposition::Cancelled => "cancelled",
        }
    }

    /// Whether a declared exclusion may target this disposition.
    pub const fn excludable(self) -> bool {
        !matches!(self, TrialDisposition::Scored)
    }
}

impl fmt::Display for TrialDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A declared exclusion. It names a disposition and a reason, and nothing else can be named.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExclusionRule {
    pub disposition: TrialDisposition,
    pub why: String,
}

impl ExclusionRule {
    /// The only constructor, and it refuses [`TrialDisposition::Scored`].
    pub fn on(
        disposition: TrialDisposition,
        why: impl Into<String>,
    ) -> Result<Self, PredeclarationError> {
        if !disposition.excludable() {
            return Err(PredeclarationError::ExclusionOfScoredTrials);
        }
        Ok(ExclusionRule {
            disposition,
            why: why.into(),
        })
    }
}

/// The level at which observations are independent.
///
/// 14.10 names "parent/cell-level aggregation"; `bioprism-atlas` calls the same idea an effective
/// size and `bioprism-scale` calls it an equivalence class. This enumeration is the plan's
/// declaration of which one it will aggregate at, not a computation over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClusteringUnit {
    /// Each trial is its own independent unit. Only honest when no instance shares a parent.
    Trial,
    /// Generated instances cluster under the parent task they were derived from.
    ParentTask,
    /// Instances cluster under the decision cell.
    DecisionCell,
    /// Instances cluster under the world they were generated from.
    World,
    /// Instances cluster under the site that produced them.
    Site,
}

impl ClusteringUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            ClusteringUnit::Trial => "trial",
            ClusteringUnit::ParentTask => "parent_task",
            ClusteringUnit::DecisionCell => "decision_cell",
            ClusteringUnit::World => "world",
            ClusteringUnit::Site => "site",
        }
    }
}

/// The analysis, declared before the results exist.
///
/// Fully serde, because a plan is meant to be published. What is not serde is the *finding* built
/// from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisPlan {
    /// The one metric a confirmatory claim may report. 14.10 makes this "primary/secondary" and
    /// the distinction only bites if there is exactly one primary.
    pub primary: String,
    #[serde(default)]
    pub secondary: BTreeSet<String>,
    #[serde(default)]
    pub exclusions: Vec<ExclusionRule>,
    #[serde(default)]
    pub seeds: Vec<u64>,
    pub retries: u32,
    /// 14.10's "stopping" field: when data collection ends, stated in advance so that it does not
    /// end when the number looks good.
    pub stopping: String,
    pub clustering: ClusteringUnit,
    /// How many independent units the analyst expects. Compared against the trial count.
    pub independent_units: usize,
    /// 14.10's "comparisons declared before result inspection". Held as declared strings; this
    /// crate does not evaluate a comparison, it records that it was named.
    #[serde(default)]
    pub comparisons: BTreeSet<String>,
}

impl AnalysisPlan {
    pub fn new(
        primary: impl Into<String>,
        clustering: ClusteringUnit,
        independent_units: usize,
        stopping: impl Into<String>,
    ) -> Self {
        AnalysisPlan {
            primary: primary.into(),
            secondary: BTreeSet::new(),
            exclusions: Vec::new(),
            seeds: Vec::new(),
            retries: 0,
            stopping: stopping.into(),
            clustering,
            independent_units,
            comparisons: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn also_reporting(mut self, metric: impl Into<String>) -> Self {
        self.secondary.insert(metric.into());
        self
    }

    #[must_use]
    pub fn excluding(mut self, rule: ExclusionRule) -> Self {
        self.exclusions.push(rule);
        self
    }

    #[must_use]
    pub fn comparing(mut self, comparison: impl Into<String>) -> Self {
        self.comparisons.insert(comparison.into());
        self
    }

    #[must_use]
    pub fn with_seeds(mut self, seeds: impl IntoIterator<Item = u64>) -> Self {
        self.seeds = seeds.into_iter().collect();
        self.seeds.sort_unstable();
        self
    }

    /// Whether `metric` may carry a confirmatory finding under this plan.
    pub fn declares(&self, metric: &str) -> bool {
        self.primary == metric || self.secondary.contains(metric)
    }

    /// Freeze the plan at `epoch`, producing the object a later finding cites.
    ///
    /// The digest is over canonical bytes, so two analysts who wrote the same plan get the same
    /// seal, and an edited plan gets a different one. 14.10 asks for predeclaration and never says
    /// what a predeclaration *is* as a value; this is the position taken.
    pub fn seal(self, epoch: Epoch) -> Result<SealedPlan, PredeclarationError> {
        if self.primary.trim().is_empty() {
            return Err(PredeclarationError::NoPrimaryMetric);
        }
        let value = serde_json::to_value(&self).map_err(|e| PredeclarationError::NotCanonical {
            detail: e.to_string(),
        })?;
        let digest =
            ContentHash::of_value(&value).map_err(|e| PredeclarationError::NotCanonical {
                detail: e.to_string(),
            })?;
        Ok(SealedPlan {
            plan: self,
            sealed_at: epoch,
            digest,
        })
    }
}

/// A plan frozen at an epoch, with a digest over its content.
///
/// Serde in both directions on purpose — this is the object that gets published, and a reader must
/// be able to parse it and recompute the digest. [`SealedPlan::verify`] does exactly that, so a
/// hand-edited plan that kept its old digest is caught rather than trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedPlan {
    plan: AnalysisPlan,
    sealed_at: Epoch,
    digest: ContentHash,
}

impl SealedPlan {
    pub fn plan(&self) -> &AnalysisPlan {
        &self.plan
    }

    pub fn sealed_at(&self) -> Epoch {
        self.sealed_at
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    /// Recompute the digest from the plan's content and compare.
    pub fn verify(&self) -> Result<(), PredeclarationError> {
        let value =
            serde_json::to_value(&self.plan).map_err(|e| PredeclarationError::NotCanonical {
                detail: e.to_string(),
            })?;
        let computed =
            ContentHash::of_value(&value).map_err(|e| PredeclarationError::NotCanonical {
                detail: e.to_string(),
            })?;
        if computed != self.digest {
            return Err(PredeclarationError::SealBroken {
                stated: self.digest.as_str().to_string(),
                computed: computed.as_str().to_string(),
            });
        }
        Ok(())
    }

    /// Report `metric` as a confirmatory finding, or say why it cannot be one.
    ///
    /// Four refusals, in order of how badly they undermine the claim: the seal is broken; the plan
    /// was sealed after the results were seen; the metric was not declared; an exclusion was applied
    /// that the plan never declared. The last is the subtle one — a plan may declare exclusions and
    /// a run may apply *different* ones, and only comparing the two catches it.
    pub fn confirm(
        &self,
        results: &ObservedResults,
        metric: &str,
    ) -> Result<ConfirmatoryFinding, PredeclarationError> {
        self.verify()?;
        if self.sealed_at >= results.observed_at {
            return Err(PredeclarationError::PlanSealedAfterResults {
                sealed: self.sealed_at,
                observed: results.observed_at,
            });
        }
        if !self.plan.declares(metric) {
            return Err(PredeclarationError::MetricNotPredeclared {
                metric: metric.to_string(),
            });
        }
        let value =
            results
                .value(metric)
                .ok_or_else(|| PredeclarationError::MetricNotMeasured {
                    metric: metric.to_string(),
                })?;

        let declared: BTreeSet<TrialDisposition> = self
            .plan
            .exclusions
            .iter()
            .map(|rule| rule.disposition)
            .collect();
        for applied in &results.excluded {
            if !declared.contains(applied) {
                return Err(PredeclarationError::UndeclaredExclusion {
                    disposition: applied.as_str(),
                });
            }
        }

        let trials = results.total_trials();
        if trials > self.plan.independent_units && self.plan.clustering == ClusteringUnit::Trial {
            return Err(PredeclarationError::ClusteringUnitAbsent {
                declared: self.plan.independent_units,
                observed: trials,
            });
        }

        Ok(ConfirmatoryFinding {
            plan_digest: self.digest.clone(),
            sealed_at: self.sealed_at,
            observed_at: results.observed_at,
            metric: metric.to_string(),
            value,
            dispositions: results.dispositions.clone(),
            excluded: results.excluded.iter().copied().collect(),
            clustering: self.plan.clustering,
            independent_units: self.plan.independent_units,
        })
    }
}

/// What a run produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedResults {
    pub observed_at: Epoch,
    values: BTreeMap<String, f64>,
    dispositions: BTreeMap<TrialDisposition, usize>,
    excluded: BTreeSet<TrialDisposition>,
}

impl ObservedResults {
    pub fn new(observed_at: Epoch) -> Self {
        ObservedResults {
            observed_at,
            values: BTreeMap::new(),
            dispositions: BTreeMap::new(),
            excluded: BTreeSet::new(),
        }
    }

    #[must_use]
    pub fn measuring(mut self, metric: impl Into<String>, value: f64) -> Self {
        self.values.insert(metric.into(), value);
        self
    }

    #[must_use]
    pub fn counting(mut self, disposition: TrialDisposition, count: usize) -> Self {
        *self.dispositions.entry(disposition).or_insert(0) += count;
        self
    }

    /// Record that trials with this disposition were dropped from the analysis.
    ///
    /// Takes a disposition rather than a rule, because what a run can report is what it did; whether
    /// it was permitted to is [`SealedPlan::confirm`]'s question.
    #[must_use]
    pub fn excluding(mut self, disposition: TrialDisposition) -> Self {
        self.excluded.insert(disposition);
        self
    }

    pub fn value(&self, metric: &str) -> Option<f64> {
        self.values.get(metric).copied()
    }

    pub fn count(&self, disposition: TrialDisposition) -> usize {
        self.dispositions.get(&disposition).copied().unwrap_or(0)
    }

    pub fn total_trials(&self) -> usize {
        self.dispositions.values().sum()
    }

    /// Trials that did not produce a score, by disposition. 14.10's missingness accounting, which
    /// this crate carries into every finding rather than summarising away.
    pub fn unscored(&self) -> Vec<(TrialDisposition, usize)> {
        self.dispositions
            .iter()
            .filter(|(d, _)| **d != TrialDisposition::Scored)
            .map(|(d, n)| (*d, *n))
            .collect()
    }
}

/// A metric reported under a plan that named it before the results existed.
///
/// Private fields, one constructor, `Serialize` and **not** `Deserialize`. The reason is
/// `bioprism_lab::CleanMeasurement`'s: a confirmatory finding that could be parsed from JSON is one
/// anybody can write, and then the seal, the epoch ordering and the exclusion comparison are
/// decoration. A reader who wants to check a published finding re-runs [`SealedPlan::confirm`]
/// against the published plan and results, which is the point.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[must_use = "a confirmatory finding is the object a causal claim cites; dropping it loses the predeclaration"]
pub struct ConfirmatoryFinding {
    plan_digest: ContentHash,
    sealed_at: Epoch,
    observed_at: Epoch,
    metric: String,
    value: f64,
    dispositions: BTreeMap<TrialDisposition, usize>,
    excluded: Vec<TrialDisposition>,
    clustering: ClusteringUnit,
    independent_units: usize,
}

impl ConfirmatoryFinding {
    pub fn metric(&self) -> &str {
        &self.metric
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn plan_digest(&self) -> &ContentHash {
        &self.plan_digest
    }

    pub fn clustering(&self) -> ClusteringUnit {
        self.clustering
    }

    pub fn independent_units(&self) -> usize {
        self.independent_units
    }

    /// Trials that did not score, carried rather than summarised. 14.10: "Missing and abstained
    /// cases cannot disappear silently."
    pub fn unscored(&self) -> Vec<(TrialDisposition, usize)> {
        self.dispositions
            .iter()
            .filter(|(d, _)| **d != TrialDisposition::Scored)
            .map(|(d, n)| (*d, *n))
            .collect()
    }

    pub fn excluded(&self) -> &[TrialDisposition] {
        &self.excluded
    }
}

impl fmt::Display for ConfirmatoryFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "confirmatory: {} = {} under plan {} sealed at {}, clustered at {}",
            self.metric,
            self.value,
            self.plan_digest.as_str(),
            self.sealed_at,
            self.clustering.as_str()
        )
    }
}

/// A metric reported without a predeclaration that named it.
///
/// Fully serde and freely constructible, because the honest thing to do with an exploratory result
/// is publish it. What does not exist is a way to turn one into a [`ConfirmatoryFinding`]: no
/// `promote`, no `From`, no constructor on the confirmatory type that takes one. A finding's
/// epistemic status is fixed by how it was obtained, and nothing later can change how it was
/// obtained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExploratoryFinding {
    pub metric: String,
    pub value: f64,
    pub observed_at: Epoch,
    /// Why this is exploratory: no plan, a plan that did not name the metric, or a plan sealed too
    /// late. Recorded so a reader knows which.
    pub because: String,
}

impl ExploratoryFinding {
    pub fn new(
        metric: impl Into<String>,
        value: f64,
        observed_at: Epoch,
        because: impl Into<String>,
    ) -> Self {
        ExploratoryFinding {
            metric: metric.into(),
            value,
            observed_at,
            because: because.into(),
        }
    }

    /// Always false. Present so that a caller holding either kind of finding can ask, and get the
    /// answer that this one is permanently not confirmatory.
    pub const fn is_confirmatory(&self) -> bool {
        false
    }
}

impl fmt::Display for ExploratoryFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "exploratory: {} = {} ({})",
            self.metric, self.value, self.because
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> AnalysisPlan {
        AnalysisPlan::new(
            "first-divergence-rate",
            ClusteringUnit::ParentTask,
            40,
            "after 400 trials",
        )
        .also_reporting("abstention-rate")
        .excluding(ExclusionRule::on(TrialDisposition::InfrastructureError, "node loss").unwrap())
        .with_seeds([3, 1, 2])
    }

    fn results() -> ObservedResults {
        ObservedResults::new(Epoch(9))
            .measuring("first-divergence-rate", 0.21)
            .measuring("recall-at-10", 0.88)
            .counting(TrialDisposition::Scored, 380)
            .counting(TrialDisposition::InfrastructureError, 20)
            .excluding(TrialDisposition::InfrastructureError)
    }

    #[test]
    fn a_metric_named_after_the_results_cannot_carry_a_confirmatory_finding() {
        let sealed = plan().seal(Epoch(4)).unwrap();
        assert!(sealed.confirm(&results(), "first-divergence-rate").is_ok());
        assert!(matches!(
            sealed.confirm(&results(), "recall-at-10"),
            Err(PredeclarationError::MetricNotPredeclared { .. })
        ));
    }

    #[test]
    fn a_plan_sealed_after_the_results_confirms_nothing() {
        let sealed = plan().seal(Epoch(9)).unwrap();
        assert!(matches!(
            sealed.confirm(&results(), "first-divergence-rate"),
            Err(PredeclarationError::PlanSealedAfterResults { .. })
        ));
        let later = plan().seal(Epoch(12)).unwrap();
        assert!(matches!(
            later.confirm(&results(), "first-divergence-rate"),
            Err(PredeclarationError::PlanSealedAfterResults { .. })
        ));
    }

    #[test]
    fn an_exclusion_of_scored_trials_has_no_spelling() {
        assert!(matches!(
            ExclusionRule::on(TrialDisposition::Scored, "they looked wrong"),
            Err(PredeclarationError::ExclusionOfScoredTrials)
        ));
        for disposition in TrialDisposition::ALL {
            assert_eq!(
                ExclusionRule::on(disposition, "why").is_ok(),
                disposition != TrialDisposition::Scored
            );
        }
    }

    #[test]
    fn an_exclusion_applied_but_never_declared_is_refused() {
        let sealed = plan().seal(Epoch(4)).unwrap();
        let sneaky = results().excluding(TrialDisposition::InvalidOutput);
        assert!(matches!(
            sealed.confirm(&sneaky, "first-divergence-rate"),
            Err(PredeclarationError::UndeclaredExclusion {
                disposition: "invalid_output"
            })
        ));
    }

    #[test]
    fn an_exploratory_finding_has_no_route_to_becoming_confirmatory() {
        let exploratory =
            ExploratoryFinding::new("recall-at-10", 0.88, Epoch(9), "not predeclared");
        assert!(!exploratory.is_confirmatory());
        let text = serde_json::to_string(&exploratory).unwrap();
        let back: ExploratoryFinding = serde_json::from_str(&text).unwrap();
        assert_eq!(back, exploratory);
    }

    #[test]
    fn a_confirmatory_finding_carries_its_unscored_trials_rather_than_summarising_them() {
        let sealed = plan().seal(Epoch(4)).unwrap();
        let finding = sealed.confirm(&results(), "first-divergence-rate").unwrap();
        assert_eq!(
            finding.unscored(),
            vec![(TrialDisposition::InfrastructureError, 20)]
        );
        assert_eq!(finding.excluded(), &[TrialDisposition::InfrastructureError]);
    }

    #[test]
    fn a_plan_whose_digest_no_longer_matches_its_content_confirms_nothing() {
        let mut sealed = plan().seal(Epoch(4)).unwrap();
        let text = serde_json::to_string(&sealed).unwrap();
        let tampered = text.replace("first-divergence-rate", "recall-at-10");
        sealed = serde_json::from_str(&tampered).unwrap();
        assert!(matches!(
            sealed.verify(),
            Err(PredeclarationError::SealBroken { .. })
        ));
        assert!(matches!(
            sealed.confirm(&results(), "recall-at-10"),
            Err(PredeclarationError::SealBroken { .. })
        ));
    }

    #[test]
    fn the_same_plan_seals_to_the_same_digest_regardless_of_declaration_order() {
        let a = plan().also_reporting("x").also_reporting("y");
        let b = plan().also_reporting("y").also_reporting("x");
        assert_eq!(
            a.seal(Epoch(1)).unwrap().digest(),
            b.seal(Epoch(1)).unwrap().digest()
        );
    }

    #[test]
    fn a_metric_cannot_be_redefined_under_a_version_it_already_holds() {
        let mut registry = MetricRegistry::new();
        let base = MetricDefinition::new(
            "first-divergence-rate",
            "1.0",
            "fraction of runs whose first divergence is at stage k or earlier",
            ContentHash::of_bytes(b"impl-a"),
            MetricDirection::LowerIsBetter,
            "measurement-council",
        );
        registry.register(base.clone()).unwrap();
        registry.register(base.clone()).unwrap();
        let redefined = MetricDefinition {
            definition_digest: ContentHash::of_bytes(b"impl-b"),
            ..base
        };
        assert!(matches!(
            registry.register(redefined),
            Err(PredeclarationError::MetricRedefinedInPlace { .. })
        ));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn trial_level_clustering_is_refused_when_trials_outnumber_independent_units() {
        let plan = AnalysisPlan::new("m", ClusteringUnit::Trial, 40, "after 400").seal(Epoch(1));
        let sealed = plan.unwrap();
        let observed = ObservedResults::new(Epoch(2))
            .measuring("m", 0.5)
            .counting(TrialDisposition::Scored, 400);
        assert!(matches!(
            sealed.confirm(&observed, "m"),
            Err(PredeclarationError::ClusteringUnitAbsent {
                declared: 40,
                observed: 400
            })
        ));
    }

    #[test]
    fn a_plan_with_no_primary_metric_cannot_be_sealed() {
        let plan = AnalysisPlan::new("  ", ClusteringUnit::ParentTask, 1, "never");
        assert!(matches!(
            plan.seal(Epoch(1)),
            Err(PredeclarationError::NoPrimaryMetric)
        ));
    }

    #[test]
    fn a_metric_declared_but_not_measured_is_not_a_zero() {
        let sealed = plan().seal(Epoch(4)).unwrap();
        let empty = ObservedResults::new(Epoch(9)).counting(TrialDisposition::Scored, 10);
        assert!(matches!(
            sealed.confirm(&empty, "first-divergence-rate"),
            Err(PredeclarationError::MetricNotMeasured { .. })
        ));
    }
}
