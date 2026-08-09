//! The capability posterior, and why a single number is hard to get out of it.
//!
//! Blueprint 00.01 lists the deliverable as "a capability posterior rather than a single score",
//! 07.12 calls the same object an Agent MRI "rather than one leaderboard row", and 07.05 sets the
//! condition under which a scalar is nonetheless permitted: "a scalar may be used for a specific
//! release gate only with its formula, rationale, and sensitivity analysis."
//!
//! Those sentences are cheap to write and cheap to violate, because someone always needs one
//! number for a dashboard. So the scalar is not a field and not a method that returns `f64`. It is
//! [`CapabilityPosterior::overall`], which takes a [`ReleaseGate`] that cannot be constructed
//! without a rationale, refuses unless every declared coverage floor was met, and returns a
//! [`GateScalar`] carrying its formula, its per-capability terms and a leave-one-out sensitivity
//! analysis. Getting a number out requires stating what you are willing to claim.
//!
//! # What the floors are for
//!
//! Each floor asks four separate questions of a capability, and they fail for different reasons:
//!
//! - `min_clusters` — how many *parent tasks* stood behind the estimate. A pack can hold a million
//!   instances and three parents.
//! - `min_effective_sample` — how much independent information those instances carried, from
//!   [`crate::cluster`]. This is the one that catches mutation-inflated packs.
//! - `max_unknown_fraction` — how much of the sample the evaluators could not read. Unknown is not
//!   failure and is not success; above a threshold it is simply not a measurement.
//! - `min_tier` — what the conclusions rested on. A capability scored entirely by judges does not
//!   satisfy a gate that asked for execution evidence, however good the number looks.
//!
//! Vetoes are checked before any of them and are not a floor at all: one outstanding safety,
//! leakage or grader-tampering veto fails the gate closed, because a veto that could be
//! outweighed by a good average would not be a veto.
//!
//! # Not implemented here
//!
//! Posterior *distributions*. The name follows the blueprint, but what this type holds is a vector
//! of clustered point estimates with their effective sample sizes and unknown shares — not a
//! density, and it does not pretend to be one. Producing genuine posteriors needs the hierarchical
//! bootstrap or Bayesian model 07.06 describes, which this crate leaves to a caller with a
//! numerics stack. Also absent: the failure atlas clustering of 07.12, conditional views by
//! context length or difficulty, cost and latency axes (07.08), and Pareto surfaces over more than
//! the capability axis — [`CapabilityPosterior::compare`] gives dominance on capabilities only.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use crate::cluster::{ClusteredEstimate, ClusteredSample};
use crate::error::EvalError;
use crate::ladder::{ScoreTier, ScoredResult};
use crate::score::{CreditPolicy, Veto};

/// One scored result, tagged with the capability it exercises and the parent it descends from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub capability: String,
    /// The parent task. Instances sharing one are not independent — see [`crate::cluster`].
    pub parent: String,
    pub result: ScoredResult,
    /// The immutable objects this observation points back at. Optional in the type and mandatory
    /// in a published report: [`unprovenanced`] finds the ones that are missing it rather than
    /// letting them through quietly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<crate::bridge::Provenance>,
}

impl Observation {
    pub fn new(
        capability: impl Into<String>,
        parent: impl Into<String>,
        result: ScoredResult,
    ) -> Self {
        Observation {
            capability: capability.into(),
            parent: parent.into(),
            result,
            provenance: None,
        }
    }

    pub fn with_provenance(mut self, provenance: crate::bridge::Provenance) -> Self {
        self.provenance = Some(provenance);
        self
    }
}

/// Observations that cannot be linked back to an immutable run.
///
/// 07.01's first invariant requires the link; this crate cannot enforce it at construction without
/// making every test fixture carry three identifiers, so it enforces it at publication time
/// instead — a caller running a release gate is expected to check this is empty.
pub fn unprovenanced(observations: &[Observation]) -> Vec<&Observation> {
    observations
        .iter()
        .filter(|observation| observation.provenance.is_none())
        .collect()
}

/// Everything known about one capability, with the two score axes kept apart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityEstimate {
    pub capability: String,
    /// Rate of unqualified passes. Unsupported passes cannot raise this.
    pub pass_rate: ClusteredEstimate,
    /// Mean partial credit. Unsupported passes do move this, capped.
    pub credit: ClusteredEstimate,
    /// Rate at which the outcome was right for *any* reason. The gap between this and `pass_rate`
    /// is the size of the "right answer, wrong reason" population, and 07.12 wants it visible.
    pub outcome_rate: ClusteredEstimate,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vetoes: Vec<Veto>,
    /// Results whose evaluators contradicted each other at the deciding tier.
    pub disputed: usize,
    pub abstained: usize,
    /// Results where a weaker evaluator was more generous than the one that decided. 07.10's
    /// reward-hacking signal, surfaced at capability level.
    pub optimistic_weak_evidence: usize,
    /// The weakest tier any conclusion in this capability rested on.
    pub weakest_tier: ScoreTier,
}

impl CapabilityEstimate {
    /// Passes that were right for a reason the evidence did not support.
    pub fn unsupported_pass_gap(&self) -> f64 {
        self.outcome_rate.mean - self.pass_rate.mean
    }

    pub fn has_outstanding_veto(&self) -> bool {
        !self.vetoes.is_empty()
    }
}

/// The capability vector. The primary output; the scalar is the exception.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityPosterior {
    pub schema_version: String,
    pub capabilities: BTreeMap<String, CapabilityEstimate>,
}

/// A coverage floor for one capability under one gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageFloor {
    pub min_clusters: usize,
    pub min_effective_sample: f64,
    pub max_unknown_fraction: f64,
    pub min_tier: ScoreTier,
    /// Relative weight in the gate's formula.
    pub weight: f64,
}

impl Default for CoverageFloor {
    /// A floor that demands very little, so that a caller who wants a lenient gate has to have
    /// written one down. There is no "no floor" option.
    fn default() -> Self {
        CoverageFloor {
            min_clusters: 2,
            min_effective_sample: 2.0,
            max_unknown_fraction: 0.2,
            min_tier: ScoreTier::Judge,
            weight: 1.0,
        }
    }
}

impl CoverageFloor {
    pub fn requiring(min_clusters: usize, min_effective_sample: f64) -> Self {
        CoverageFloor {
            min_clusters,
            min_effective_sample,
            ..CoverageFloor::default()
        }
    }

    pub fn grounded(mut self) -> Self {
        self.min_tier = ScoreTier::Execution;
        self
    }

    pub fn weighted(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// A named gate: the only thing that can turn a capability vector into a number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseGate {
    pub gate: String,
    /// Why collapsing the vector is defensible *for this decision*. Required by 07.05 and by
    /// [`ReleaseGate::new`], which refuses an empty one.
    pub rationale: String,
    pub formula: String,
    pub floors: BTreeMap<String, CoverageFloor>,
}

impl ReleaseGate {
    /// Construct a gate. Fails when the rationale is empty.
    pub fn new(gate: impl Into<String>, rationale: impl Into<String>) -> Result<Self, EvalError> {
        let gate = gate.into();
        let rationale = rationale.into();
        if rationale.trim().is_empty() {
            return Err(EvalError::GateWithoutRationale { gate });
        }
        Ok(ReleaseGate {
            gate,
            rationale,
            formula: "weighted mean of per-capability full-pass rates, cluster-balanced".to_string(),
            floors: BTreeMap::new(),
        })
    }

    pub fn require(mut self, capability: impl Into<String>, floor: CoverageFloor) -> Self {
        self.floors.insert(capability.into(), floor);
        self
    }

    pub fn with_formula(mut self, formula: impl Into<String>) -> Self {
        self.formula = formula.into();
        self
    }
}

/// A scalar that carries everything 07.05 requires before it may be quoted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateScalar {
    pub gate: String,
    pub value: f64,
    pub formula: String,
    pub rationale: String,
    /// Per-capability terms, so the value can be recomputed by hand.
    pub terms: Vec<(String, f64, f64)>,
    /// Leave-one-capability-out values. A gate whose number moves a long way when one capability
    /// is dropped is a gate measuring that capability, whatever it is called.
    pub sensitivity: Vec<(String, f64)>,
    /// The weakest evidence anywhere under this number.
    pub weakest_tier: ScoreTier,
    /// The smallest effective sample size anywhere under this number.
    pub min_effective_sample: f64,
}

impl GateScalar {
    /// The largest absolute move any single capability's removal causes.
    pub fn largest_sensitivity(&self) -> f64 {
        self.sensitivity
            .iter()
            .map(|(_, value)| (value - self.value).abs())
            .fold(0.0, f64::max)
    }
}

/// The result of comparing two posteriors without collapsing either.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "dominance", rename_all = "snake_case")]
pub enum Dominance {
    /// Better or equal on every shared capability, strictly better on at least one.
    Dominates,
    DominatedBy,
    /// Within tolerance everywhere.
    Equivalent,
    /// The usual answer. Better here, worse there — and no scalar in this crate will pick.
    Incomparable {
        better: Vec<String>,
        worse: Vec<String>,
        /// Capabilities one side or the other did not measure, or measured too thinly to order.
        uncertain: Vec<String>,
    },
}

impl CapabilityPosterior {
    /// Aggregate scored results into a capability vector.
    ///
    /// Uninformative conclusions — unknown, disputed, unexamined justification — are pushed as
    /// unknown rather than as zero, and surface as [`ClusteredEstimate::unknown_fraction`].
    /// Abstentions are counted but kept out of the pass-rate denominator: an agent that declined
    /// has not failed the task, and folding abstention into failure punishes exactly the behaviour
    /// a calibration-aware evaluation wants to encourage.
    pub fn build(observations: &[Observation], policy: &CreditPolicy) -> Result<Self, EvalError> {
        let mut pass: BTreeMap<String, ClusteredSample> = BTreeMap::new();
        let mut credit: BTreeMap<String, ClusteredSample> = BTreeMap::new();
        let mut outcome: BTreeMap<String, ClusteredSample> = BTreeMap::new();
        let mut extras: BTreeMap<String, (Vec<Veto>, usize, usize, usize, ScoreTier)> =
            BTreeMap::new();

        for observation in observations {
            let capability = observation.capability.as_str();
            let conclusion = observation.result.conclusion;

            let pass_sample = pass
                .entry(capability.to_string())
                .or_insert_with(|| ClusteredSample::new(format!("{capability}::pass_rate")));
            let outcome_sample = outcome
                .entry(capability.to_string())
                .or_insert_with(|| ClusteredSample::new(format!("{capability}::outcome_rate")));
            let credit_sample = credit
                .entry(capability.to_string())
                .or_insert_with(|| ClusteredSample::new(format!("{capability}::credit")));

            if conclusion.is_uninformative() || conclusion == crate::score::Conclusion::Abstained {
                pass_sample.push_unknown(&observation.parent);
                outcome_sample.push_unknown(&observation.parent);
            } else {
                pass_sample.push(&observation.parent, f64::from(u8::from(conclusion.is_full_pass())));
                outcome_sample.push(
                    &observation.parent,
                    f64::from(u8::from(conclusion.outcome_was_correct())),
                );
            }

            match observation.result.credit(policy).fraction {
                Some(fraction) => credit_sample.push(&observation.parent, fraction),
                None => credit_sample.push_unknown(&observation.parent),
            }

            let entry = extras.entry(capability.to_string()).or_insert((
                Vec::new(),
                0,
                0,
                0,
                ScoreTier::Deterministic,
            ));
            entry.0.extend(observation.result.vetoes.iter().cloned());
            if observation.result.needs_resolution() {
                entry.1 += 1;
            }
            if conclusion == crate::score::Conclusion::Abstained {
                entry.2 += 1;
            }
            if observation.result.has_optimistic_weak_evidence() {
                entry.3 += 1;
            }
            entry.4 = entry.4.min(observation.result.deciding_tier);
        }

        let mut capabilities = BTreeMap::new();
        for (capability, pass_sample) in pass {
            let credit_sample = credit.remove(&capability).expect("built in step");
            let outcome_sample = outcome.remove(&capability).expect("built in step");
            let (vetoes, disputed, abstained, optimistic, weakest_tier) =
                extras.remove(&capability).expect("built in step");
            capabilities.insert(
                capability.clone(),
                CapabilityEstimate {
                    capability: capability.clone(),
                    pass_rate: pass_sample.estimate()?,
                    credit: credit_sample.estimate()?,
                    outcome_rate: outcome_sample.estimate()?,
                    vetoes,
                    disputed,
                    abstained,
                    optimistic_weak_evidence: optimistic,
                    weakest_tier,
                },
            );
        }

        Ok(CapabilityPosterior {
            schema_version: crate::EVALENGINE_SCHEMA_VERSION.to_string(),
            capabilities,
        })
    }

    pub fn get(&self, capability: &str) -> Option<&CapabilityEstimate> {
        self.capabilities.get(capability)
    }

    /// Every capability holding an outstanding veto.
    pub fn vetoed(&self) -> impl Iterator<Item = &CapabilityEstimate> {
        self.capabilities
            .values()
            .filter(|estimate| estimate.has_outstanding_veto())
    }

    /// Collapse to one number for one named gate, or explain why not.
    ///
    /// Checks run per capability in a fixed order — existence, veto, parents, effective sample,
    /// unknown share, tier — and the first failure is returned. Only the first, deliberately: a
    /// list of eight problems invites triage, and a gate is not partially passed.
    pub fn overall(&self, gate: &ReleaseGate) -> Result<GateScalar, EvalError> {
        if gate.rationale.trim().is_empty() {
            return Err(EvalError::GateWithoutRationale {
                gate: gate.gate.clone(),
            });
        }
        if gate.floors.is_empty() {
            return Err(EvalError::GateWithoutCoverageFloors {
                gate: gate.gate.clone(),
            });
        }

        let mut terms = Vec::new();
        let mut weakest_tier = ScoreTier::Deterministic;
        let mut min_effective = f64::INFINITY;

        for (capability, floor) in &gate.floors {
            let estimate =
                self.capabilities
                    .get(capability)
                    .ok_or_else(|| EvalError::CapabilityUnobserved {
                        gate: gate.gate.clone(),
                        capability: capability.clone(),
                    })?;

            if let Some(veto) = estimate.vetoes.first() {
                return Err(EvalError::VetoOutstanding {
                    gate: gate.gate.clone(),
                    capability: capability.clone(),
                    kind: veto.kind.as_str().to_string(),
                    detail: veto.detail.clone(),
                });
            }
            if estimate.pass_rate.clusters < floor.min_clusters {
                return Err(EvalError::ClusterFloorUnmet {
                    gate: gate.gate.clone(),
                    capability: capability.clone(),
                    observed: estimate.pass_rate.clusters,
                    required: floor.min_clusters,
                });
            }
            if estimate.pass_rate.effective_sample_size < floor.min_effective_sample {
                return Err(EvalError::EffectiveSampleFloorUnmet {
                    gate: gate.gate.clone(),
                    capability: capability.clone(),
                    observed: estimate.pass_rate.effective_sample_size,
                    required: floor.min_effective_sample,
                });
            }
            if estimate.pass_rate.unknown_fraction > floor.max_unknown_fraction {
                return Err(EvalError::UnknownFractionExceeded {
                    gate: gate.gate.clone(),
                    capability: capability.clone(),
                    observed: estimate.pass_rate.unknown_fraction,
                    tolerated: floor.max_unknown_fraction,
                });
            }
            if estimate.weakest_tier < floor.min_tier {
                return Err(EvalError::TierFloorUnmet {
                    gate: gate.gate.clone(),
                    capability: capability.clone(),
                    weakest: estimate.weakest_tier.to_string(),
                    required: floor.min_tier.to_string(),
                });
            }

            weakest_tier = weakest_tier.min(estimate.weakest_tier);
            min_effective = min_effective.min(estimate.pass_rate.effective_sample_size);
            terms.push((capability.clone(), estimate.pass_rate.mean, floor.weight));
        }

        let value = weighted_mean(&terms).ok_or_else(|| EvalError::GateWithoutCoverageFloors {
            gate: gate.gate.clone(),
        })?;

        let sensitivity = terms
            .iter()
            .map(|(dropped, _, _)| {
                let remaining: Vec<(String, f64, f64)> = terms
                    .iter()
                    .filter(|(capability, _, _)| capability != dropped)
                    .cloned()
                    .collect();
                (dropped.clone(), weighted_mean(&remaining).unwrap_or(value))
            })
            .collect();

        Ok(GateScalar {
            gate: gate.gate.clone(),
            value,
            formula: gate.formula.clone(),
            rationale: gate.rationale.clone(),
            terms,
            sensitivity,
            weakest_tier,
            min_effective_sample: min_effective,
        })
    }

    /// Compare two posteriors capability by capability, without a scalar.
    ///
    /// A capability either side measured too thinly to order — effective sample below
    /// `min_effective` — is `uncertain` and forces [`Dominance::Incomparable`] rather than being
    /// quietly treated as a tie. So does a capability only one side measured.
    pub fn compare(
        &self,
        other: &CapabilityPosterior,
        tolerance: f64,
        min_effective: f64,
    ) -> Dominance {
        let names: BTreeSet<&String> = self
            .capabilities
            .keys()
            .chain(other.capabilities.keys())
            .collect();

        let mut better = Vec::new();
        let mut worse = Vec::new();
        let mut uncertain = Vec::new();

        for name in names {
            match (self.capabilities.get(name), other.capabilities.get(name)) {
                (Some(left), Some(right)) => {
                    if left.pass_rate.effective_sample_size < min_effective
                        || right.pass_rate.effective_sample_size < min_effective
                    {
                        uncertain.push(name.clone());
                        continue;
                    }
                    let delta = left.pass_rate.mean - right.pass_rate.mean;
                    if delta > tolerance {
                        better.push(name.clone());
                    } else if delta < -tolerance {
                        worse.push(name.clone());
                    }
                }
                _ => uncertain.push(name.clone()),
            }
        }

        if !uncertain.is_empty() || (!better.is_empty() && !worse.is_empty()) {
            return Dominance::Incomparable {
                better,
                worse,
                uncertain,
            };
        }
        match (better.is_empty(), worse.is_empty()) {
            (true, true) => Dominance::Equivalent,
            (false, true) => Dominance::Dominates,
            (true, false) => Dominance::DominatedBy,
            (false, false) => unreachable!("handled above"),
        }
    }

    /// A capability table for a human. Pass rate and outcome rate sit next to each other on
    /// purpose: the gap between them is the unsupported-pass population.
    pub fn to_markdown(&self) -> String {
        use std::fmt::Write as _;
        let mut text = String::new();
        let _ = writeln!(
            text,
            "| Capability | Pass | Outcome | Credit | Parents | Instances | Effective n | Unknown | Evidence |"
        );
        let _ = writeln!(text, "|---|---:|---:|---:|---:|---:|---:|---:|---|");
        for estimate in self.capabilities.values() {
            let _ = writeln!(
                text,
                "| {} | {:.3} | {:.3} | {:.3} | {} | {} | {:.2} | {:.0}% | {} |",
                estimate.capability,
                estimate.pass_rate.mean,
                estimate.outcome_rate.mean,
                estimate.credit.mean,
                estimate.pass_rate.clusters,
                estimate.pass_rate.instances,
                estimate.pass_rate.effective_sample_size,
                estimate.pass_rate.unknown_fraction * 100.0,
                estimate.weakest_tier
            );
        }
        for estimate in self.capabilities.values() {
            if estimate.unsupported_pass_gap() > 0.0 {
                let _ = writeln!(
                    text,
                    "\n- `{}` reached the right outcome {:.0}% more often than it passed. That gap \
                     is runs that were right for a reason the evidence does not support.",
                    estimate.capability,
                    estimate.unsupported_pass_gap() * 100.0
                );
            }
            if estimate.optimistic_weak_evidence > 0 {
                let _ = writeln!(
                    text,
                    "\n- `{}` has {} result(s) where a weaker evaluator was more generous than the \
                     one that decided.",
                    estimate.capability, estimate.optimistic_weak_evidence
                );
            }
            for veto in &estimate.vetoes {
                let _ = writeln!(
                    text,
                    "\n- `{}` holds an outstanding **{}** veto from `{}`: {}",
                    estimate.capability,
                    veto.kind.as_str(),
                    veto.evaluator,
                    veto.detail
                );
            }
        }
        let _ = writeln!(
            text,
            "\nThis vector is the result. A scalar exists only for a named release gate, with its \
             formula, rationale and sensitivity attached."
        );
        text
    }
}

fn weighted_mean(terms: &[(String, f64, f64)]) -> Option<f64> {
    let total_weight: f64 = terms.iter().map(|(_, _, weight)| weight).sum();
    if total_weight <= 0.0 {
        return None;
    }
    Some(
        terms
            .iter()
            .map(|(_, value, weight)| value * weight)
            .sum::<f64>()
            / total_weight,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ladder::{compose, Contribution, UnknownPolicy};
    use crate::score::{Conclusion, VetoKind};

    fn scored(id: &str, tier: ScoreTier, conclusion: Conclusion) -> ScoredResult {
        compose(
            id,
            &[Contribution::new(tier, "evaluator@1", conclusion)],
            &UnknownPolicy::Block,
        )
        .expect("composes")
    }

    fn posterior(rows: &[(&str, &str, Conclusion)]) -> CapabilityPosterior {
        let observations: Vec<Observation> = rows
            .iter()
            .enumerate()
            .map(|(index, (capability, parent, conclusion))| {
                Observation::new(
                    *capability,
                    *parent,
                    scored(
                        &format!("r{index}"),
                        ScoreTier::Deterministic,
                        *conclusion,
                    ),
                )
            })
            .collect();
        CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds")
    }

    fn lenient_gate() -> ReleaseGate {
        ReleaseGate::new("ship", "one number is needed for the release checklist")
            .expect("rationale given")
            .require("planning", CoverageFloor::requiring(2, 2.0))
    }

    #[test]
    fn a_release_gate_cannot_be_declared_without_a_rationale() {
        let err = ReleaseGate::new("ship", "   ").unwrap_err();
        assert_eq!(
            err,
            EvalError::GateWithoutRationale {
                gate: "ship".to_string()
            }
        );
    }

    #[test]
    fn an_overall_scalar_is_refused_when_no_coverage_floors_were_declared() {
        let posterior = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Pass),
        ]);
        let gate = ReleaseGate::new("ship", "needed for the checklist").expect("rationale");
        assert_eq!(
            posterior.overall(&gate).unwrap_err(),
            EvalError::GateWithoutCoverageFloors {
                gate: "ship".to_string()
            }
        );
    }

    #[test]
    fn an_overall_scalar_is_refused_when_the_parent_floor_is_unmet() {
        let posterior = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p1", Conclusion::Fail),
        ]);
        assert_eq!(
            posterior.overall(&lenient_gate()).unwrap_err(),
            EvalError::ClusterFloorUnmet {
                gate: "ship".to_string(),
                capability: "planning".to_string(),
                observed: 1,
                required: 2,
            }
        );
    }

    #[test]
    fn an_overall_scalar_is_refused_when_effective_sample_size_is_too_small() {
        let mut rows = Vec::new();
        for index in 0..40 {
            rows.push(("planning", if index < 20 { "p1" } else { "p2" }, Conclusion::Pass));
        }
        let posterior = posterior(&rows);
        let gate = ReleaseGate::new("ship", "checklist")
            .expect("rationale")
            .require("planning", CoverageFloor::requiring(2, 10.0));
        assert!(matches!(
            posterior.overall(&gate).unwrap_err(),
            EvalError::EffectiveSampleFloorUnmet { .. }
        ));
    }

    #[test]
    fn an_overall_scalar_is_refused_when_a_veto_is_outstanding() {
        let vetoed = compose(
            "r0",
            &[Contribution::new(
                ScoreTier::Deterministic,
                "leak-scan@1",
                Conclusion::Pass,
            )
            .with_veto(Veto::new(VetoKind::Safety, "leak-scan@1", "unsafe tool call"))],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        let observations = vec![
            Observation::new("planning", "p1", vetoed),
            Observation::new(
                "planning",
                "p2",
                scored("r1", ScoreTier::Deterministic, Conclusion::Pass),
            ),
        ];
        let posterior =
            CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
        assert!(matches!(
            posterior.overall(&lenient_gate()).unwrap_err(),
            EvalError::VetoOutstanding { .. }
        ));
    }

    #[test]
    fn an_overall_scalar_is_refused_when_the_evidence_is_weaker_than_the_gate_demanded() {
        let observations = vec![
            Observation::new(
                "planning",
                "p1",
                scored("r0", ScoreTier::Judge, Conclusion::Pass),
            ),
            Observation::new(
                "planning",
                "p2",
                scored("r1", ScoreTier::Deterministic, Conclusion::Pass),
            ),
        ];
        let posterior =
            CapabilityPosterior::build(&observations, &CreditPolicy::default()).expect("builds");
        let gate = ReleaseGate::new("ship", "checklist")
            .expect("rationale")
            .require("planning", CoverageFloor::requiring(2, 2.0).grounded());
        assert!(matches!(
            posterior.overall(&gate).unwrap_err(),
            EvalError::TierFloorUnmet { .. }
        ));
    }

    #[test]
    fn an_overall_scalar_is_refused_for_a_capability_that_was_never_measured() {
        let posterior = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Pass),
        ]);
        let gate = lenient_gate().require("tool_use", CoverageFloor::requiring(2, 2.0));
        assert_eq!(
            posterior.overall(&gate).unwrap_err(),
            EvalError::CapabilityUnobserved {
                gate: "ship".to_string(),
                capability: "tool_use".to_string(),
            }
        );
    }

    #[test]
    fn a_granted_scalar_carries_its_formula_rationale_and_sensitivity() {
        let posterior = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Fail),
            ("tool_use", "p3", Conclusion::Pass),
            ("tool_use", "p4", Conclusion::Pass),
        ]);
        let gate = ReleaseGate::new("ship", "release checklist needs one number")
            .expect("rationale")
            .require("planning", CoverageFloor::requiring(2, 2.0))
            .require("tool_use", CoverageFloor::requiring(2, 2.0));
        let scalar = posterior.overall(&gate).expect("floors met");
        assert!((scalar.value - 0.75).abs() < 1e-9);
        assert_eq!(scalar.terms.len(), 2);
        assert_eq!(scalar.sensitivity.len(), 2);
        assert!(!scalar.rationale.is_empty());
        assert!(scalar.largest_sensitivity() > 0.0);
    }

    #[test]
    fn unsupported_passes_raise_the_outcome_rate_but_not_the_pass_rate() {
        let posterior = posterior(&[
            ("planning", "p1", Conclusion::UnsupportedPass),
            ("planning", "p2", Conclusion::Pass),
        ]);
        let estimate = posterior.get("planning").expect("present");
        assert!((estimate.pass_rate.mean - 0.5).abs() < 1e-9);
        assert!((estimate.outcome_rate.mean - 1.0).abs() < 1e-9);
        assert!((estimate.unsupported_pass_gap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn unknown_results_leave_the_denominator_instead_of_counting_as_failures() {
        let posterior = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Unknown),
        ]);
        let estimate = posterior.get("planning").expect("present");
        assert!((estimate.pass_rate.mean - 1.0).abs() < 1e-9);
        assert_eq!(estimate.pass_rate.unknown_instances, 1);
        assert!((estimate.pass_rate.unknown_fraction - 0.5).abs() < 1e-9);
    }

    #[test]
    fn two_agents_that_trade_wins_across_capabilities_are_incomparable() {
        let left = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Pass),
            ("tool_use", "p3", Conclusion::Fail),
            ("tool_use", "p4", Conclusion::Fail),
        ]);
        let right = posterior(&[
            ("planning", "p1", Conclusion::Fail),
            ("planning", "p2", Conclusion::Fail),
            ("tool_use", "p3", Conclusion::Pass),
            ("tool_use", "p4", Conclusion::Pass),
        ]);
        match left.compare(&right, 0.01, 2.0) {
            Dominance::Incomparable { better, worse, .. } => {
                assert_eq!(better, vec!["planning".to_string()]);
                assert_eq!(worse, vec!["tool_use".to_string()]);
            }
            other => panic!("expected incomparable, got {other:?}"),
        }
    }

    #[test]
    fn a_capability_only_one_side_measured_blocks_a_dominance_claim() {
        let left = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Pass),
            ("tool_use", "p3", Conclusion::Pass),
            ("tool_use", "p4", Conclusion::Pass),
        ]);
        let right = posterior(&[
            ("planning", "p1", Conclusion::Fail),
            ("planning", "p2", Conclusion::Fail),
        ]);
        assert!(matches!(
            left.compare(&right, 0.01, 2.0),
            Dominance::Incomparable { .. }
        ));
    }

    #[test]
    fn dominance_is_granted_only_when_no_shared_capability_regressed() {
        let left = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Pass),
        ]);
        let right = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Fail),
        ]);
        assert_eq!(left.compare(&right, 0.01, 2.0), Dominance::Dominates);
        assert_eq!(right.compare(&left, 0.01, 2.0), Dominance::DominatedBy);
    }

    #[test]
    fn a_posterior_round_trips_through_json() {
        let posterior = posterior(&[
            ("planning", "p1", Conclusion::Pass),
            ("planning", "p2", Conclusion::Fail),
        ]);
        let text = serde_json::to_string(&posterior).expect("serialize");
        let back: CapabilityPosterior = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(posterior, back);
    }
}
