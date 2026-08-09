//! Information-directed selection: which instance to run next, and why.
//!
//! Blueprint 08.03 states the objective as "expected reduction in entropy or posterior variance
//! over targeted capability or effect parameters divided by predicted cost, with coverage and
//! quality constraints", and 43.15 gives the same shape as a ratio of expected decision-value to
//! multidimensional cost. This module implements the posterior-variance form:
//!
//! ```text
//!   score(candidate) = expected_variance_reduction(clustered posterior of its capability)
//!                      * marginal_independent_weight(trials already spent on its parent, rho)
//!                      / cost
//! ```
//!
//! Three properties are worth stating because they are what make it defensible rather than
//! merely plausible.
//!
//! **The base term is exact.** `variance / (mass + 1)` is the closed-form expected variance
//! reduction of a Beta-Bernoulli update, not a sampled approximation. See
//! [`crate::beta::BetaPosterior::expected_variance_reduction`].
//!
//! **The diversity penalty is the same `rho` that widens the interval.** 08.03 asks separately
//! for a "diversity penalty" that penalises cells correlated with already-run parents. Rather
//! than a second tuned term, the weight is the marginal effective-sample-size contribution
//! implied by the intraclass correlation already estimated for the inference. A scheduler and an
//! estimator that disagree about how much a repeat is worth will happily spend a budget acquiring
//! evidence the estimator then discards.
//!
//! **Coverage is a gate, not a penalty.** While *any* capability is below a floor, only
//! candidates that reduce some capability's highest-priority shortfall are eligible, and the
//! acquisition score then chooses within that set. A soft penalty large enough to guarantee
//! coverage is indistinguishable from a gate; one small enough to be traded away does not
//! guarantee anything. The gate is global across capabilities for the same reason: a
//! per-capability gate only reorders candidates inside a capability, and lets the capability with
//! the most diffuse posterior absorb a budget another capability needed to reach its floor.
//!
//! Exploration is **not** implemented. 08.03 reserves probability mass for under-modelled domains
//! on the grounds that "a greedy scheduler can entrench a wrong posterior"; this selector is
//! purely greedy within the coverage gate, and its defence against entrenchment is the coverage
//! floor alone. Randomised exploration would also make a suite non-reproducible without threading
//! the seed through every selection, which is a design decision, not an oversight — but it is a
//! gap, and a panel run against a badly misspecified prior will feel it.

use crate::beta::BetaPrior;
use crate::cluster::{marginal_independent_weight, Icc};
use crate::coverage::{CoveragePolicy, CoverageStatus, Shortfall};
use crate::error::AdaptiveError;
use crate::id::{CapabilityId, InstanceId, ParentId};
use crate::ledger::TrialLedger;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// An instance the panel could run next.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub instance: InstanceId,
    pub capability: CapabilityId,
    pub parent: ParentId,
    /// Predicted cost, in the caller's budget unit. Must be finite and positive.
    pub cost: f64,
}

impl Candidate {
    pub fn new(
        instance: InstanceId,
        capability: CapabilityId,
        parent: ParentId,
        cost: f64,
    ) -> Result<Self, AdaptiveError> {
        if !cost.is_finite() || cost <= 0.0 {
            return Err(AdaptiveError::InvalidCost {
                instance: instance.to_string(),
                cost,
            });
        }
        Ok(Candidate {
            instance,
            capability,
            parent,
            cost,
        })
    }
}

/// Knobs the selector exposes, all of which change behaviour rather than just performance.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SelectionConfig {
    /// The intraclass correlation to assume for a capability whose own `rho` is not yet
    /// estimable.
    ///
    /// A *policy choice*, not a measurement, and it is deliberately pessimistic. At cold start
    /// there is no evidence either way, and the cost of assuming independence wrongly (a panel
    /// that spends its whole budget inside two parent families) is much higher than the cost of
    /// assuming dependence wrongly (a panel that spreads across parents it did not need to).
    pub assumed_icc: f64,
    /// Whether unmet coverage floors gate the candidate pool. Turning this off makes the
    /// selector purely greedy and is only sensible for scheduler-versus-scheduler experiments.
    pub coverage_first: bool,
    /// How many rejected candidates to keep in the audit record.
    pub runners_up: usize,
}

impl Default for SelectionConfig {
    fn default() -> Self {
        SelectionConfig {
            assumed_icc: 0.5,
            coverage_first: true,
            runners_up: 3,
        }
    }
}

/// A scored candidate, as it appeared to the selector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredCandidate {
    pub instance: InstanceId,
    pub capability: CapabilityId,
    pub parent: ParentId,
    pub score: f64,
    /// The exact Beta-Bernoulli expected variance reduction for the capability.
    pub expected_variance_reduction: f64,
    /// The independent-information discount for this parent's existing trials.
    pub independence_weight: f64,
    pub cost: f64,
    pub parent_trials_before: usize,
}

/// Why this instance, out of everything that was available.
///
/// Blueprint 08.08 asks a selection record to carry "posterior/model version, candidate snapshot,
/// feature values, constraints, acquisition scores, sampling probabilities, seed, chosen batch,
/// and rejected candidates". This record carries the scores, the constraint state, the objective
/// terms and the top rejected candidates. It does **not** carry the full candidate snapshot —
/// that belongs in the run store, and duplicating a registry-sized list into every selection
/// would make the audit trail larger than the evidence. There are no sampling probabilities to
/// record because selection is deterministic, and no seed because nothing here is sampled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionRecord {
    pub chosen: ScoredCandidate,
    /// Candidates that survived the eligibility and coverage filters.
    pub eligible: usize,
    /// Candidates removed because their instance already carries scored evidence.
    pub already_run: usize,
    /// Candidates removed by an active coverage gate.
    pub coverage_gated_out: usize,
    /// Which capability's floor forced the gate, if one did.
    pub gated_by: Option<CoverageGate>,
    pub runners_up: Vec<ScoredCandidate>,
    /// The intraclass correlation used for the chosen candidate's capability, and where it came
    /// from.
    pub icc_used: f64,
    pub icc_source: IccSource,
    pub caveat: String,
}

/// Where the `rho` used in scoring came from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IccSource {
    /// Estimated from this capability's own trials.
    Estimated,
    /// Every trial so far came from a distinct parent; `rho` is irrelevant and taken as zero.
    NoClustering,
    /// Not estimable yet; [`SelectionConfig::assumed_icc`] was used.
    Assumed,
}

/// The coverage shortfall that restricted the pool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageGate {
    pub capability: CapabilityId,
    pub shortfall: Shortfall,
}

impl SelectionRecord {
    /// Content hash of the record, over canonical JSON.
    ///
    /// Lets a run store reference a selection without copying it, and lets a replay prove it
    /// reproduced the same decision rather than a similar-looking one.
    pub fn digest(&self) -> Result<ContentHash, AdaptiveError> {
        let value =
            serde_json::to_value(self).map_err(|e| AdaptiveError::Canonical(e.to_string()))?;
        ContentHash::of_value(&value).map_err(|e| AdaptiveError::Canonical(e.to_string()))
    }
}

const SELECTION_CAVEAT: &str = "Scores are greedy and myopic: they value one trial at a time \
                                against the current posterior, with no lookahead and no reserved \
                                exploration mass. Cost is the caller's predicted cost, not a \
                                measured one.";

struct CapabilityContext {
    expected_variance_reduction: f64,
    rho: f64,
    icc_source: IccSource,
    status: CoverageStatus,
    gate: Option<Shortfall>,
    /// Scored trials already spent on each parent, indexed rather than rescanned.
    ///
    /// Selection asks this question once per candidate per pick; deriving it from the ledger each
    /// time makes a panel run quadratic in its own budget.
    parent_trials: BTreeMap<ParentId, usize>,
}

/// Highest-priority unmet shortfall for a capability.
///
/// Sentinels first, because a missing longitudinal anchor makes every comparison across runs
/// unverifiable; then parent breadth, because it is what the interval depends on; then raw trial
/// count; then dominance. Fixing the order makes the scheduler's behaviour predictable and its
/// records comparable across runs.
fn binding_shortfall(status: &CoverageStatus) -> Option<Shortfall> {
    let priority = |s: &Shortfall| match s {
        Shortfall::Sentinel { .. } => 0,
        Shortfall::QualifyingParents { .. } => 1,
        Shortfall::Trials { .. } => 2,
        Shortfall::ParentDominance { .. } => 3,
    };
    status
        .shortfalls
        .iter()
        .min_by_key(|s| priority(s))
        .cloned()
}

fn relieves(
    shortfall: &Shortfall,
    candidate: &Candidate,
    parent_trials: usize,
    policy: &CoveragePolicy,
) -> bool {
    match shortfall {
        Shortfall::Sentinel { parent, .. } => &candidate.parent == parent,
        Shortfall::QualifyingParents { .. } => {
            parent_trials < policy.min_trials_per_parent.max(1)
        }
        Shortfall::Trials { .. } => true,
        Shortfall::ParentDominance { parent, .. } => &candidate.parent != parent,
    }
}

fn context(
    capability: &CapabilityId,
    ledger: &TrialLedger,
    prior: &BetaPrior,
    policy: &CoveragePolicy,
    config: &SelectionConfig,
) -> Result<CapabilityContext, AdaptiveError> {
    let summary = ledger.summary(capability);
    let posterior = summary.clustered_posterior(prior)?;
    let icc = summary.icc();
    let (rho, icc_source) = match icc {
        Icc::Estimated { rho, .. } => (rho, IccSource::Estimated),
        Icc::NoClustering => (0.0, IccSource::NoClustering),
        Icc::Unidentifiable { .. } => (config.assumed_icc, IccSource::Assumed),
    };
    let status = policy.status(ledger, capability);
    let gate = if config.coverage_first {
        binding_shortfall(&status)
    } else {
        None
    };
    Ok(CapabilityContext {
        expected_variance_reduction: posterior.expected_variance_reduction(),
        rho: rho.clamp(0.0, 1.0),
        icc_source,
        status,
        gate,
        parent_trials: summary
            .clusters
            .iter()
            .map(|c| (c.parent.clone(), c.trials))
            .collect(),
    })
}

/// The total order selection uses: score descending, then instance identifier, then capability.
///
/// Written as an explicit predicate over `f64::total_cmp` rather than a `PartialOrd` sort so that
/// two candidates whose scores are bit-identical always resolve the same way, on every platform
/// and in every run. A suite whose selection record cannot be replayed proves nothing.
fn ranks_before(a_score: f64, a: &Candidate, b_score: f64, b: &Candidate) -> bool {
    match b_score.total_cmp(&a_score) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => {
            (a.instance.as_str(), a.capability.as_str())
                < (b.instance.as_str(), b.capability.as_str())
        }
    }
}

fn scored_candidate(
    candidate: &Candidate,
    score: f64,
    parent_trials: usize,
    ctx: &CapabilityContext,
) -> ScoredCandidate {
    ScoredCandidate {
        instance: candidate.instance.clone(),
        capability: candidate.capability.clone(),
        parent: candidate.parent.clone(),
        score,
        expected_variance_reduction: ctx.expected_variance_reduction,
        independence_weight: marginal_independent_weight(parent_trials, ctx.rho),
        cost: candidate.cost,
        parent_trials_before: parent_trials,
    }
}

/// Chooses the single most informative eligible instance per unit cost.
///
/// Ties in the score are broken by instance identifier, then capability identifier, both
/// ascending. A suite that re-runs must select the same instances in the same order or its
/// selection record proves nothing.
pub fn select_next(
    candidates: &[Candidate],
    ledger: &TrialLedger,
    prior: &BetaPrior,
    policy: &CoveragePolicy,
    config: &SelectionConfig,
) -> Result<SelectionRecord, AdaptiveError> {
    let batch = select_batch(candidates, ledger, prior, policy, config, 1)?;
    batch
        .into_iter()
        .next()
        .ok_or(AdaptiveError::NoCandidates)
}

/// Chooses `size` instances, applying the diversity penalty *within* the batch.
///
/// Blueprint 08.07 wants batches so worker startup does not dominate, and insists that "the
/// planner does not count dispatch as evidence". So a provisional selection increments the
/// parent's trial count — which is known the moment it is dispatched — and does **not** touch the
/// posterior, which is not. The consequence is exactly what it should be: a batch spreads across
/// parents rather than piling onto whichever capability currently looks most uncertain.
///
/// Coverage gates within a batch are evaluated against *committed* evidence only, so a batch can
/// over-select for a floor that its own earlier picks already satisfy. That is the safe
/// direction, and correcting it would require re-deriving coverage from provisional dispatch,
/// which is the thing 08.07 forbids counting.
///
/// Two implementation choices are worth naming. The coverage gate is evaluated once across all
/// capabilities rather than per capability, because a per-capability gate only reorders
/// candidates inside a capability and lets the one with the most diffuse posterior absorb a
/// budget another capability needed to reach its floor. And the scored candidates are held as
/// a small ranked prefix of `(score, candidate index, parent trials)` rather than a sorted copy
/// of the pool, because rebuilding and sorting a registry-sized `Vec` for every pick makes a
/// panel run quadratic in the registry and allocation-bound long before it is
/// arithmetic-bound.
pub fn select_batch(
    candidates: &[Candidate],
    ledger: &TrialLedger,
    prior: &BetaPrior,
    policy: &CoveragePolicy,
    config: &SelectionConfig,
    size: usize,
) -> Result<Vec<SelectionRecord>, AdaptiveError> {
    if candidates.is_empty() {
        return Err(AdaptiveError::NoCandidates);
    }
    for candidate in candidates {
        if !candidate.cost.is_finite() || candidate.cost <= 0.0 {
            return Err(AdaptiveError::InvalidCost {
                instance: candidate.instance.to_string(),
                cost: candidate.cost,
            });
        }
    }

    let mut contexts: BTreeMap<CapabilityId, CapabilityContext> = BTreeMap::new();
    for candidate in candidates {
        if !contexts.contains_key(&candidate.capability) {
            contexts.insert(
                candidate.capability.clone(),
                context(&candidate.capability, ledger, prior, policy, config)?,
            );
        }
    }

    let coverage_pending = contexts.values().any(|c| c.gate.is_some());

    let mut provisional: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut taken: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut records = Vec::new();
    let keep = config.runners_up + 1;

    for _ in 0..size {
        let mut already_run = 0usize;
        let mut gated_out = 0usize;
        let mut eligible = 0usize;
        let mut gate_hit: Option<CoverageGate> = None;
        let mut best: Vec<(f64, usize, usize)> = Vec::with_capacity(keep + 1);

        for (index, candidate) in candidates.iter().enumerate() {
            if ledger.has_scored(&candidate.capability, &candidate.instance)
                || taken.contains(&(candidate.capability.as_str(), candidate.instance.as_str()))
            {
                already_run += 1;
                continue;
            }
            let ctx = &contexts[&candidate.capability];
            if coverage_pending && ctx.gate.is_none() {
                gated_out += 1;
                continue;
            }
            let parent_trials = ctx
                .parent_trials
                .get(&candidate.parent)
                .copied()
                .unwrap_or(0)
                + provisional
                    .get(&(candidate.capability.as_str(), candidate.parent.as_str()))
                    .copied()
                    .unwrap_or(0);

            if let Some(shortfall) = &ctx.gate {
                if !relieves(shortfall, candidate, parent_trials, policy) {
                    gated_out += 1;
                    continue;
                }
                gate_hit.get_or_insert_with(|| CoverageGate {
                    capability: candidate.capability.clone(),
                    shortfall: shortfall.clone(),
                });
            }

            eligible += 1;
            let weight = marginal_independent_weight(parent_trials, ctx.rho);
            let score = ctx.expected_variance_reduction * weight / candidate.cost;
            let position = best.partition_point(|(held_score, held_index, _)| {
                ranks_before(*held_score, &candidates[*held_index], score, candidate)
            });
            if position < keep {
                best.insert(position, (score, index, parent_trials));
                best.truncate(keep);
            }
        }

        if best.is_empty() {
            if let Some((capability, ctx)) = contexts.iter().find(|(_, c)| c.gate.is_some()) {
                return Err(AdaptiveError::CoverageUnsatisfiable {
                    capability: capability.to_string(),
                    shortfalls: ctx.status.describe(),
                });
            }
            break;
        }

        let (score, index, parent_trials) = best[0];
        let candidate = &candidates[index];
        let ctx = &contexts[&candidate.capability];
        let chosen = scored_candidate(candidate, score, parent_trials, ctx);
        let runners_up: Vec<ScoredCandidate> = best[1..]
            .iter()
            .map(|(score, index, parent_trials)| {
                let candidate = &candidates[*index];
                scored_candidate(candidate, *score, *parent_trials, &contexts[&candidate.capability])
            })
            .collect();

        *provisional
            .entry((candidate.capability.as_str(), candidate.parent.as_str()))
            .or_insert(0) += 1;
        taken.insert((
            candidate.capability.as_str(),
            candidate.instance.as_str(),
        ));

        records.push(SelectionRecord {
            eligible,
            already_run,
            coverage_gated_out: gated_out,
            gated_by: gate_hit,
            icc_used: ctx.rho,
            icc_source: ctx.icc_source,
            chosen,
            runners_up,
            caveat: SELECTION_CAVEAT.to_string(),
        });
    }

    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{Outcome, Trial};

    fn candidate(instance: &str, capability: &str, parent: &str, cost: f64) -> Candidate {
        Candidate::new(
            InstanceId::parse(instance).unwrap(),
            CapabilityId::parse(capability).unwrap(),
            ParentId::parse(parent).unwrap(),
            cost,
        )
        .unwrap()
    }

    fn open_policy() -> CoveragePolicy {
        CoveragePolicy {
            min_trials_per_capability: 0,
            min_parents_per_capability: 0,
            min_trials_per_parent: 1,
            max_parent_share: None,
            sentinels: BTreeMap::new(),
        }
    }

    fn record(ledger: &mut TrialLedger, capability: &str, instance: &str, parent: &str, pass: bool) {
        ledger
            .record(
                Trial::new(
                    CapabilityId::parse(capability).unwrap(),
                    InstanceId::parse(instance).unwrap(),
                    ParentId::parse(parent).unwrap(),
                    if pass { Outcome::Pass } else { Outcome::Fail },
                    1.0,
                )
                .unwrap(),
            )
            .unwrap();
    }

    #[test]
    fn selection_is_reproducible_and_breaks_ties_by_instance_identifier() {
        // Three identical candidates: same capability, same fresh parent situation, same cost.
        let candidates = vec![
            candidate("i-c", "cap", "p3", 1.0),
            candidate("i-a", "cap", "p1", 1.0),
            candidate("i-b", "cap", "p2", 1.0),
        ];
        let ledger = TrialLedger::new();
        let first = select_next(
            &candidates,
            &ledger,
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
        )
        .unwrap();
        let again = select_next(
            &candidates,
            &ledger,
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
        )
        .unwrap();
        assert_eq!(first.chosen.instance.as_str(), "i-a");
        assert_eq!(first.digest().unwrap(), again.digest().unwrap());
    }

    #[test]
    fn a_cheaper_candidate_wins_when_the_information_is_equal() {
        let candidates = vec![
            candidate("i-a", "cap", "p1", 10.0),
            candidate("i-b", "cap", "p2", 1.0),
        ];
        let chosen = select_next(
            &candidates,
            &TrialLedger::new(),
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
        )
        .unwrap();
        assert_eq!(chosen.chosen.instance.as_str(), "i-b");
        assert!(chosen.chosen.score > chosen.runners_up[0].score);
    }

    #[test]
    fn an_already_sampled_parent_loses_to_a_fresh_one_of_equal_cost() {
        // Parents that are internally unanimous but disagree with each other: rho estimates near
        // one, so a ninth trial on p1 is worth almost nothing and a first trial on p9 is worth
        // everything. Note the tie-break would have preferred "i-a"; only the independence weight
        // overturns it.
        let mut ledger = TrialLedger::new();
        for i in 0..6 {
            record(&mut ledger, "cap", &format!("a-{i}"), "p1", true);
            record(&mut ledger, "cap", &format!("b-{i}"), "p2", false);
            record(&mut ledger, "cap", &format!("c-{i}"), "p3", true);
        }

        let candidates = vec![
            candidate("i-a", "cap", "p1", 1.0),
            candidate("i-z", "cap", "p9", 1.0),
        ];
        let chosen = select_next(
            &candidates,
            &ledger,
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
        )
        .unwrap();
        assert_eq!(chosen.chosen.instance.as_str(), "i-z");
        assert!(chosen.chosen.independence_weight > chosen.runners_up[0].independence_weight);
    }

    #[test]
    fn an_instance_already_scored_is_never_proposed_again() {
        let mut ledger = TrialLedger::new();
        record(&mut ledger, "cap", "i-a", "p1", true);
        let candidates = vec![
            candidate("i-a", "cap", "p1", 1.0),
            candidate("i-b", "cap", "p1", 5.0),
        ];
        let chosen = select_next(
            &candidates,
            &ledger,
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
        )
        .unwrap();
        assert_eq!(chosen.chosen.instance.as_str(), "i-b");
        assert_eq!(chosen.already_run, 1);
    }

    #[test]
    fn a_coverage_floor_gates_the_pool_even_when_another_capability_scores_higher() {
        // "easy" already has plenty of evidence and a tiny remaining variance; "hard" has none.
        // Without the gate the greedy score would keep feeding whichever capability is more
        // uncertain, and a floor on `hard` would never bind on a per-capability basis. The gate
        // makes it bind.
        let mut ledger = TrialLedger::new();
        for i in 0..40 {
            record(&mut ledger, "easy", &format!("e{i}"), &format!("p{}", i % 8), true);
        }
        let policy = CoveragePolicy {
            min_trials_per_capability: 10,
            min_parents_per_capability: 3,
            min_trials_per_parent: 2,
            max_parent_share: None,
            sentinels: BTreeMap::new(),
        };
        let candidates = vec![
            candidate("e-new", "easy", "p0", 1.0),
            candidate("h-new", "hard", "q0", 1.0),
        ];
        let chosen = select_next(
            &candidates,
            &ledger,
            &BetaPrior::default(),
            &policy,
            &SelectionConfig::default(),
        )
        .unwrap();
        assert_eq!(chosen.chosen.capability.as_str(), "hard");
        assert!(chosen.gated_by.is_some());
    }

    #[test]
    fn a_sentinel_parent_outranks_every_information_argument() {
        let mut policy = open_policy();
        policy.min_trials_per_parent = 2;
        policy.sentinels.insert(
            CapabilityId::parse("cap").unwrap(),
            [ParentId::parse("golden").unwrap()].into_iter().collect(),
        );
        let candidates = vec![
            candidate("cheap-and-fresh", "cap", "p1", 0.01),
            candidate("expensive-sentinel", "cap", "golden", 100.0),
        ];
        let chosen = select_next(
            &candidates,
            &TrialLedger::new(),
            &BetaPrior::default(),
            &policy,
            &SelectionConfig::default(),
        )
        .unwrap();
        assert_eq!(chosen.chosen.parent.as_str(), "golden");
        assert!(matches!(
            chosen.gated_by.as_ref().map(|g| &g.shortfall),
            Some(Shortfall::Sentinel { .. })
        ));
    }

    #[test]
    fn an_unsatisfiable_coverage_floor_is_an_error_and_not_a_quiet_fallback() {
        let mut policy = open_policy();
        policy.sentinels.insert(
            CapabilityId::parse("cap").unwrap(),
            [ParentId::parse("missing-parent").unwrap()]
                .into_iter()
                .collect(),
        );
        let candidates = vec![candidate("i-a", "cap", "p1", 1.0)];
        let outcome = select_next(
            &candidates,
            &TrialLedger::new(),
            &BetaPrior::default(),
            &policy,
            &SelectionConfig::default(),
        );
        assert!(matches!(
            outcome,
            Err(AdaptiveError::CoverageUnsatisfiable { .. })
        ));
    }

    #[test]
    fn a_batch_spreads_across_parents_instead_of_repeating_the_best_scoring_one() {
        let candidates: Vec<Candidate> = (0..5)
            .flat_map(|p| {
                (0..4).map(move |i| {
                    candidate(&format!("i-{p}-{i}"), "cap", &format!("p{p}"), 1.0)
                })
            })
            .collect();
        let batch = select_batch(
            &candidates,
            &TrialLedger::new(),
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
            5,
        )
        .unwrap();
        let parents: BTreeSet<&str> = batch
            .iter()
            .map(|r| r.chosen.parent.as_str())
            .collect();
        assert_eq!(batch.len(), 5);
        assert_eq!(parents.len(), 5, "a batch of five reused a parent");
    }

    #[test]
    fn a_batch_never_dispatches_the_same_instance_twice() {
        let candidates: Vec<Candidate> = (0..12)
            .map(|i| candidate(&format!("i{i:02}"), "cap", &format!("p{}", i % 3), 1.0))
            .collect();
        let batch = select_batch(
            &candidates,
            &TrialLedger::new(),
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
            12,
        )
        .unwrap();
        let instances: BTreeSet<&str> = batch
            .iter()
            .map(|r| r.chosen.instance.as_str())
            .collect();
        assert_eq!(instances.len(), 12);
    }

    #[test]
    fn a_batch_larger_than_the_pool_returns_the_pool_rather_than_repeating() {
        let candidates = vec![
            candidate("i0", "cap", "p0", 1.0),
            candidate("i1", "cap", "p1", 1.0),
        ];
        let batch = select_batch(
            &candidates,
            &TrialLedger::new(),
            &BetaPrior::default(),
            &open_policy(),
            &SelectionConfig::default(),
            10,
        )
        .unwrap();
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn a_pool_whose_every_candidate_has_already_run_is_an_error_and_not_a_repeat() {
        let mut ledger = TrialLedger::new();
        record(&mut ledger, "cap", "i-a", "p1", true);
        record(&mut ledger, "cap", "i-b", "p2", false);
        let candidates = vec![
            candidate("i-a", "cap", "p1", 1.0),
            candidate("i-b", "cap", "p2", 1.0),
        ];
        assert!(matches!(
            select_next(
                &candidates,
                &ledger,
                &BetaPrior::default(),
                &open_policy(),
                &SelectionConfig::default()
            ),
            Err(AdaptiveError::NoCandidates)
        ));
    }

    #[test]
    fn an_empty_candidate_set_is_an_error() {
        assert!(matches!(
            select_next(
                &[],
                &TrialLedger::new(),
                &BetaPrior::default(),
                &open_policy(),
                &SelectionConfig::default()
            ),
            Err(AdaptiveError::NoCandidates)
        ));
    }

    #[test]
    fn a_non_positive_candidate_cost_is_rejected_before_any_scoring() {
        let bad = Candidate {
            instance: InstanceId::parse("i").unwrap(),
            capability: CapabilityId::parse("c").unwrap(),
            parent: ParentId::parse("p").unwrap(),
            cost: 0.0,
        };
        assert!(matches!(
            select_next(
                &[bad],
                &TrialLedger::new(),
                &BetaPrior::default(),
                &open_policy(),
                &SelectionConfig::default()
            ),
            Err(AdaptiveError::InvalidCost { .. })
        ));
    }
}
