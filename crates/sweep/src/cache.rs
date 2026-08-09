//! Distributed execution: cache keys that cannot be short, and attempts that cannot be lost.
//!
//! Implements blueprint 05.10 (Distributed Execution, Caching and Recovery). `bioprism-runtime`
//! owns 05.01–05.09; this is the module that scales them across workers, and it is the one place in
//! §05 where the failure is not a wrong answer but a *reused* one.
//!
//! # A cache key that omits a dimension is a correctness bug, not a performance bug
//!
//! 05.10: "Include all semantic dependencies: task, state, architecture, model resolution, tool
//! schema, evaluator, policy, environment, and relevant secrets or fixture versions."
//!
//! Nine dimensions. A key built from eight of them returns a hit across a change in the ninth, and
//! the run that consumes the hit is scored as though it had executed under the new condition. There
//! is no downstream check that catches this, because the result is well-formed.
//!
//! So [`CacheKey`] has no constructor that takes fewer than nine values. [`CacheKeyBuilder::build`]
//! names every dimension that was not set, there is no `Default`, and there is no
//! `CacheKey::from_str`. Adding a tenth dimension to [`Dimension`] invalidates every existing key
//! — which is correct, and is why the enum is closed.
//!
//! # Three ways for work not to have finished
//!
//! 05.10: "Lost workers produce incomplete attempts; the scheduler may retry under declared
//! policy." [`Attempt`] distinguishes [`Attempt::Complete`], [`Attempt::Incomplete`] (a worker
//! took a lease and stopped reporting) and [`Attempt::NotAttempted`] (no lease was ever taken).
//! [`AttemptLedger::completion`] refuses to compute a completion rate that treats the third as the
//! second, because a plan with a hundred trials of which sixty completed, ten died and thirty were
//! never dispatched is in a different state from one where forty died.
//!
//! # An outage cannot quietly change the experiment
//!
//! 05.10: "Provider outages trigger pause, alternative provider selection **only when comparability
//! allows**, or explicit incomplete status. They do not silently change models or environments."
//!
//! [`respond_to_outage`] returns [`OutageResponse::Substitute`] only when handed a
//! [`ComparabilityDeclaration`]; with none, the choice is between pausing and marking the
//! experiment incomplete, and the caller must say which. A substitution therefore always has a
//! stated basis attached to it in the record.
//!
//! # What is not implemented
//!
//! No workers, no leases over a wire, no heartbeats, no storage. The worker protocol of 05.10 is a
//! distributed-systems design and this crate has no network — what is here is the part that stays
//! true regardless of transport: what a key must contain, what an attempt can be, and what an
//! outage may do. Speculative execution is modelled as a permission ([`may_speculate`]) rather than
//! as a scheduler.

use std::collections::BTreeMap;

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{require_nonempty, SweepError};

/// The nine semantic dependencies of a cached execution, in 05.10's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Task,
    State,
    Architecture,
    ModelResolution,
    ToolSchema,
    Evaluator,
    Policy,
    Environment,
    /// 05.10's "relevant secrets or fixture versions" — the version, never the secret.
    FixtureOrSecretVersion,
}

impl Dimension {
    pub const ALL: [Dimension; 9] = [
        Dimension::Task,
        Dimension::State,
        Dimension::Architecture,
        Dimension::ModelResolution,
        Dimension::ToolSchema,
        Dimension::Evaluator,
        Dimension::Policy,
        Dimension::Environment,
        Dimension::FixtureOrSecretVersion,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Dimension::Task => "task",
            Dimension::State => "state",
            Dimension::Architecture => "architecture",
            Dimension::ModelResolution => "model_resolution",
            Dimension::ToolSchema => "tool_schema",
            Dimension::Evaluator => "evaluator",
            Dimension::Policy => "policy",
            Dimension::Environment => "environment",
            Dimension::FixtureOrSecretVersion => "fixture_or_secret_version",
        }
    }
}

/// A key over all nine dimensions. There is no way to build a shorter one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CacheKey(String);

impl CacheKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Collects the nine dimensions and refuses to produce a key without them.
#[derive(Debug, Clone, Default)]
pub struct CacheKeyBuilder {
    values: BTreeMap<&'static str, String>,
}

impl CacheKeyBuilder {
    pub fn new() -> Self {
        CacheKeyBuilder::default()
    }

    pub fn set(mut self, dimension: Dimension, value: impl Into<String>) -> Self {
        self.values.insert(dimension.as_str(), value.into());
        self
    }

    /// Build, or name every dimension that was not set.
    pub fn build(self) -> Result<CacheKey, SweepError> {
        let missing: Vec<String> = Dimension::ALL
            .into_iter()
            .filter(|d| !self.values.contains_key(d.as_str()))
            .map(|d| d.as_str().to_string())
            .collect();
        if !missing.is_empty() {
            return Err(SweepError::MissingField { what: "CacheKey", fields: missing });
        }
        let digest = ContentHash::of_value(&json!(self.values))?;
        Ok(CacheKey(digest.as_str().to_string()))
    }
}

/// What a trial does to the world, as far as speculation is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrialProfile {
    pub trial_id: String,
    /// True only when the trial performs no effect outside its sandbox.
    pub side_effect_free: bool,
}

/// Whether a straggler may be duplicated.
///
/// 05.10: speculative execution "may duplicate stragglers only for side-effect-free trials".
/// Duplicating a trial that sends an email sends two emails; there is no compensation layer here
/// and 05.08's own docs say compensation is not implemented, so the answer is simply no.
pub fn may_speculate(trial: &TrialProfile) -> bool {
    trial.side_effect_free
}

/// What happened to one dispatched-or-not trial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Attempt {
    /// Finished and finalised with an evidence digest.
    Complete { evidence: ContentHash },
    /// A lease was taken and never finalised.
    Incomplete { reason: String },
    /// No lease was ever taken. Distinct from `Incomplete` in what it says about the worker pool
    /// and in what a retry would cost.
    NotAttempted,
}

/// A tally of attempts with three bins and no rate that hides one of them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AttemptLedger {
    attempts: BTreeMap<String, Attempt>,
    /// 05.10: "duplicates remain recorded for reliability analysis".
    duplicates: BTreeMap<String, u32>,
}

/// How far along an experiment is, without collapsing the three states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    pub complete: u32,
    pub incomplete: u32,
    pub not_attempted: u32,
}

impl Completion {
    pub fn total(&self) -> u32 {
        self.complete + self.incomplete + self.not_attempted
    }

    /// Whether the experiment finished. False while anything is outstanding *in either sense*.
    pub fn finished(&self) -> bool {
        self.incomplete == 0 && self.not_attempted == 0 && self.complete > 0
    }
}

impl AttemptLedger {
    pub fn new() -> Self {
        AttemptLedger::default()
    }

    /// Record a trial as dispatched-but-unfinished. Idempotent per trial id.
    pub fn lease(&mut self, trial_id: impl Into<String>) {
        let id = trial_id.into();
        self.attempts
            .entry(id)
            .or_insert(Attempt::Incomplete { reason: "leased, not yet finalised".to_string() });
    }

    /// Record a trial nobody has taken. Never overwrites a lease.
    pub fn enqueue(&mut self, trial_id: impl Into<String>) {
        self.attempts.entry(trial_id.into()).or_insert(Attempt::NotAttempted);
    }

    pub fn finalize(&mut self, trial_id: &str, evidence: ContentHash) {
        self.attempts.insert(trial_id.to_string(), Attempt::Complete { evidence });
    }

    /// A worker stopped reporting. The reason is required — "lost" without a cause cannot be
    /// distinguished from a trial the scheduler abandoned on purpose.
    pub fn abandon(&mut self, trial_id: &str, reason: impl Into<String>) -> Result<(), SweepError> {
        let reason = reason.into();
        require_nonempty(&reason, "AttemptLedger::abandon", "reason")?;
        self.attempts.insert(trial_id.to_string(), Attempt::Incomplete { reason });
        Ok(())
    }

    pub fn record_duplicate(&mut self, trial_id: &str) {
        *self.duplicates.entry(trial_id.to_string()).or_insert(0) += 1;
    }

    pub fn duplicates_of(&self, trial_id: &str) -> u32 {
        self.duplicates.get(trial_id).copied().unwrap_or(0)
    }

    pub fn attempt(&self, trial_id: &str) -> Option<&Attempt> {
        self.attempts.get(trial_id)
    }

    pub fn completion(&self) -> Completion {
        let mut c = Completion { complete: 0, incomplete: 0, not_attempted: 0 };
        for attempt in self.attempts.values() {
            match attempt {
                Attempt::Complete { .. } => c.complete += 1,
                Attempt::Incomplete { .. } => c.incomplete += 1,
                Attempt::NotAttempted => c.not_attempted += 1,
            }
        }
        c
    }
}

/// A named cache of finished executions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionCache {
    entries: BTreeMap<String, ContentHash>,
}

/// The two answers a lookup can give.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum Lookup {
    Hit { evidence: ContentHash },
    Miss,
}

impl ExecutionCache {
    pub fn new() -> Self {
        ExecutionCache::default()
    }

    pub fn insert(&mut self, key: &CacheKey, evidence: ContentHash) {
        self.entries.insert(key.as_str().to_string(), evidence);
    }

    pub fn lookup(&self, key: &CacheKey) -> Lookup {
        match self.entries.get(key.as_str()) {
            Some(evidence) => Lookup::Hit { evidence: evidence.clone() },
            None => Lookup::Miss,
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A stated basis for treating two providers' results as belonging to the same experiment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparabilityDeclaration {
    pub from_provider: String,
    pub to_provider: String,
    pub basis: String,
}

impl ComparabilityDeclaration {
    pub fn new(
        from_provider: impl Into<String>,
        to_provider: impl Into<String>,
        basis: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let basis = basis.into();
        require_nonempty(&basis, "ComparabilityDeclaration", "basis")?;
        Ok(ComparabilityDeclaration {
            from_provider: from_provider.into(),
            to_provider: to_provider.into(),
            basis,
        })
    }
}

/// What the controller wants to do about an outage, before comparability is consulted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutagePreference {
    Pause,
    Substitute,
    MarkIncomplete,
}

/// What the controller is permitted to do about an outage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "response")]
pub enum OutageResponse {
    Pause,
    /// Only reachable with a declaration, which travels in the response so the record carries it.
    Substitute { declaration: ComparabilityDeclaration },
    MarkIncomplete,
}

/// Decide what an outage may do.
///
/// A `Substitute` preference with no declaration is an error rather than a silent downgrade to
/// `Pause`: the controller asked for something it may not have, and swapping in a different
/// behaviour without saying so is the failure mode the module exists to prevent.
pub fn respond_to_outage(
    preference: OutagePreference,
    declaration: Option<ComparabilityDeclaration>,
) -> Result<OutageResponse, SweepError> {
    match (preference, declaration) {
        (OutagePreference::Pause, _) => Ok(OutageResponse::Pause),
        (OutagePreference::MarkIncomplete, _) => Ok(OutageResponse::MarkIncomplete),
        (OutagePreference::Substitute, Some(declaration)) => {
            Ok(OutageResponse::Substitute { declaration })
        }
        (OutagePreference::Substitute, None) => Err(SweepError::UndeclaredPrecondition {
            operation: "provider substitution during an outage",
            declaration: "a comparability declaration between the two providers",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_key() -> CacheKeyBuilder {
        Dimension::ALL
            .into_iter()
            .fold(CacheKeyBuilder::new(), |b, d| b.set(d, d.as_str()))
    }

    fn evidence(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    #[test]
    fn the_dimension_list_is_the_blueprints_nine() {
        assert_eq!(Dimension::ALL.len(), 9);
    }

    #[test]
    fn a_key_missing_any_dimension_cannot_be_built_and_the_gap_is_named() {
        assert!(full_key().build().is_ok());
        let partial = CacheKeyBuilder::new().set(Dimension::Task, "t");
        match partial.build().unwrap_err() {
            SweepError::MissingField { fields, .. } => {
                assert_eq!(fields.len(), 8);
                assert!(fields.contains(&"evaluator".to_string()));
            }
            other => panic!("expected MissingField, got {other:?}"),
        }
    }

    #[test]
    fn changing_any_one_of_the_nine_dimensions_changes_the_key() {
        let base = full_key().build().unwrap();
        for dimension in Dimension::ALL {
            let altered = full_key().set(dimension, "changed").build().unwrap();
            assert_ne!(base, altered, "key did not change when {dimension:?} changed");
        }
    }

    #[test]
    fn the_same_nine_values_always_give_the_same_key() {
        assert_eq!(full_key().build().unwrap(), full_key().build().unwrap());
    }

    #[test]
    fn a_cache_hit_requires_the_identical_key() {
        let mut cache = ExecutionCache::new();
        let key = full_key().build().unwrap();
        cache.insert(&key, evidence("e1"));
        assert_eq!(cache.lookup(&key), Lookup::Hit { evidence: evidence("e1") });
        let other = full_key().set(Dimension::Policy, "policy-v2").build().unwrap();
        assert_eq!(cache.lookup(&other), Lookup::Miss);
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
    }

    #[test]
    fn only_a_side_effect_free_trial_may_be_speculatively_duplicated() {
        assert!(may_speculate(&TrialProfile {
            trial_id: "t1".into(),
            side_effect_free: true
        }));
        assert!(!may_speculate(&TrialProfile {
            trial_id: "t2".into(),
            side_effect_free: false
        }));
    }

    #[test]
    fn an_unattempted_trial_is_not_an_incomplete_one() {
        let mut ledger = AttemptLedger::new();
        ledger.enqueue("t1");
        ledger.lease("t2");
        ledger.abandon("t2", "worker heartbeat lost").unwrap();
        ledger.lease("t3");
        ledger.finalize("t3", evidence("e"));
        let completion = ledger.completion();
        assert_eq!(completion.complete, 1);
        assert_eq!(completion.incomplete, 1);
        assert_eq!(completion.not_attempted, 1);
        assert_eq!(completion.total(), 3);
        assert!(!completion.finished());
    }

    #[test]
    fn an_experiment_is_finished_only_when_nothing_is_outstanding_in_either_sense() {
        let mut ledger = AttemptLedger::new();
        ledger.lease("t1");
        ledger.finalize("t1", evidence("e"));
        assert!(ledger.completion().finished());
        ledger.enqueue("t2");
        assert!(!ledger.completion().finished());
    }

    #[test]
    fn abandoning_a_trial_requires_a_reason() {
        let mut ledger = AttemptLedger::new();
        ledger.lease("t1");
        assert!(ledger.abandon("t1", "  ").is_err());
    }

    #[test]
    fn enqueueing_never_overwrites_an_existing_lease() {
        let mut ledger = AttemptLedger::new();
        ledger.lease("t1");
        ledger.enqueue("t1");
        assert!(matches!(ledger.attempt("t1"), Some(Attempt::Incomplete { .. })));
    }

    #[test]
    fn speculative_duplicates_stay_on_the_record() {
        let mut ledger = AttemptLedger::new();
        ledger.lease("t1");
        ledger.record_duplicate("t1");
        ledger.record_duplicate("t1");
        ledger.finalize("t1", evidence("e"));
        assert_eq!(ledger.duplicates_of("t1"), 2);
        assert_eq!(ledger.duplicates_of("t2"), 0);
    }

    #[test]
    fn an_outage_cannot_substitute_a_provider_without_a_comparability_declaration() {
        let err = respond_to_outage(OutagePreference::Substitute, None).unwrap_err();
        assert!(matches!(err, SweepError::UndeclaredPrecondition { .. }));
    }

    #[test]
    fn a_substitution_carries_its_basis_into_the_record() {
        let declaration =
            ComparabilityDeclaration::new("p1", "p2", "same weights, same decoding parameters")
                .unwrap();
        match respond_to_outage(OutagePreference::Substitute, Some(declaration)).unwrap() {
            OutageResponse::Substitute { declaration } => {
                assert!(declaration.basis.contains("same weights"));
            }
            other => panic!("expected Substitute, got {other:?}"),
        }
    }

    #[test]
    fn pausing_and_marking_incomplete_need_no_declaration() {
        assert_eq!(
            respond_to_outage(OutagePreference::Pause, None).unwrap(),
            OutageResponse::Pause
        );
        assert_eq!(
            respond_to_outage(OutagePreference::MarkIncomplete, None).unwrap(),
            OutageResponse::MarkIncomplete
        );
    }

    #[test]
    fn a_comparability_declaration_without_a_basis_is_refused() {
        assert!(ComparabilityDeclaration::new("p1", "p2", "").is_err());
    }
}
