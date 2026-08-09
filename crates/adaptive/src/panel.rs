//! The panel: the five pieces wired into one auditable object.
//!
//! Blueprint 08.01's responsibilities in order — "Maintain a belief over architecture
//! capabilities. Choose the next cell or panel. Guarantee required coverage and safety checks.
//! Stop when evidence is sufficient. Remain reproducible and auditable." — map onto
//! [`AdaptivePanel::record`], [`AdaptivePanel::select_next`], [`AdaptivePanel::coverage`],
//! [`AdaptivePanel::stopping_verdict`] and [`AdaptivePanel::audit`].
//!
//! The one asymmetry worth pointing at: [`AdaptivePanel::summary`] will hand back raw counts for
//! any capability, but [`AdaptivePanel::estimate`] refuses below the coverage floor. Counts are
//! facts and are always preserved; an estimate is a claim, and a claim the panel is not entitled
//! to make should not be obtainable with a caveat attached, because caveats do not survive being
//! copied into a slide.
//!
//! 08.01 also specifies a fallback this crate honours by construction: "When models are
//! uncalibrated, use stratified fixed panels. Adaptivity is earned through validation, not assumed
//! superior." Setting [`SelectionConfig::coverage_first`] with floors that consume the whole
//! budget degenerates this panel into exactly that stratified fixed panel, and 08.08's deployment
//! gate — non-inferiority against the fixed reference panel before adaptive scheduling may control
//! a release — is a *program*, not a function call, and is not implemented here.

use crate::beta::BetaPrior;
use crate::cluster::{BootstrapConfig, ClusterSummary};
use crate::coverage::{CoveragePolicy, CoverageStatus};
use crate::error::AdaptiveError;
use crate::estimate::{
    naive_probability_of_superiority, probability_of_superiority, CapabilityEstimate,
};
use crate::id::CapabilityId;
use crate::ledger::{Trial, TrialLedger};
use crate::select::{select_batch, select_next, Candidate, SelectionConfig, SelectionRecord};
use crate::stopping::{self, StoppingRule, StoppingVerdict};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

/// Everything that changes what a panel concludes, in one serializable place.
///
/// Serializable because 08.01 requires "Every selection records candidate set, objective terms,
/// probabilities, constraints, random seed, and stopping reason" — a stopping reason is not
/// interpretable without the rule that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PanelConfig {
    pub prior: BetaPrior,
    pub coverage: CoveragePolicy,
    pub stopping: StoppingRule,
    pub selection: SelectionConfig,
    /// Seed and draw count for the second-opinion cluster bootstrap. `None` skips it.
    pub bootstrap: Option<BootstrapConfig>,
}

impl PanelConfig {
    pub fn credibility(&self) -> f64 {
        self.stopping.credibility
    }
}

/// An adaptive evaluation panel over one architecture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptivePanel {
    config: PanelConfig,
    ledger: TrialLedger,
}

impl AdaptivePanel {
    pub fn new(config: PanelConfig) -> Self {
        AdaptivePanel {
            config,
            ledger: TrialLedger::new(),
        }
    }

    pub fn config(&self) -> &PanelConfig {
        &self.config
    }

    pub fn ledger(&self) -> &TrialLedger {
        &self.ledger
    }

    /// Records an executed trial. Refuses a second scored trial on an instance.
    pub fn record(&mut self, trial: Trial) -> Result<(), AdaptiveError> {
        self.ledger.record(trial)
    }

    /// The next instance to run.
    pub fn select_next(&self, candidates: &[Candidate]) -> Result<SelectionRecord, AdaptiveError> {
        select_next(
            candidates,
            &self.ledger,
            &self.config.prior,
            &self.config.coverage,
            &self.config.selection,
        )
    }

    /// The next `size` instances, diversified across parents within the batch.
    pub fn select_batch(
        &self,
        candidates: &[Candidate],
        size: usize,
    ) -> Result<Vec<SelectionRecord>, AdaptiveError> {
        select_batch(
            candidates,
            &self.ledger,
            &self.config.prior,
            &self.config.coverage,
            &self.config.selection,
            size,
        )
    }

    /// Raw scored counts grouped by parent. Always available: counts are facts.
    pub fn summary(&self, capability: &CapabilityId) -> ClusterSummary {
        self.ledger.summary(capability)
    }

    pub fn coverage(&self, capability: &CapabilityId) -> CoverageStatus {
        self.config.coverage.status(&self.ledger, capability)
    }

    /// The reportable estimate, or a typed refusal.
    ///
    /// Fails with [`AdaptiveError::UnknownCapability`] when nothing was ever run, and with
    /// [`AdaptiveError::CoverageFloorNotMet`] when something was run but not enough of it.
    /// The two are distinguished because they call for different actions: supply candidates, or
    /// supply *different* candidates.
    pub fn estimate(&self, capability: &CapabilityId) -> Result<CapabilityEstimate, AdaptiveError> {
        let summary = self.ledger.summary(capability);
        if summary.is_empty() {
            return Err(AdaptiveError::UnknownCapability {
                capability: capability.to_string(),
            });
        }
        self.coverage(capability).require_met()?;
        CapabilityEstimate::from_summary(
            capability.clone(),
            &summary,
            self.ledger.abstentions(capability),
            &self.config.prior,
            self.config.credibility(),
            self.config.bootstrap.as_ref(),
        )
    }

    pub fn stopping_verdict(
        &self,
        capability: &CapabilityId,
    ) -> Result<StoppingVerdict, AdaptiveError> {
        let summary = self.ledger.summary(capability);
        if summary.is_empty() {
            return Err(AdaptiveError::UnknownCapability {
                capability: capability.to_string(),
            });
        }
        stopping::evaluate(
            capability,
            &summary,
            &self.coverage(capability),
            &self.config.prior,
            &self.config.stopping,
        )
    }

    /// Whether every capability that has been touched has stopped.
    pub fn finished(&self) -> Result<bool, AdaptiveError> {
        for capability in self.ledger.capabilities() {
            if !self.stopping_verdict(&capability)?.stop {
                return Ok(false);
            }
        }
        Ok(!self.ledger.is_empty())
    }

    /// A paired capability comparison, on the clustered posteriors.
    ///
    /// Both capabilities must be reportable. This is where Gate 4's "the same meaningful
    /// capability differences" acquires a number, and where a panel is allowed to answer "we
    /// cannot tell from ten parents".
    pub fn compare(
        &self,
        left: &CapabilityId,
        right: &CapabilityId,
    ) -> Result<Comparison, AdaptiveError> {
        let a = self.estimate(left)?;
        let b = self.estimate(right)?;
        Ok(Comparison {
            left: a.capability.clone(),
            right: b.capability.clone(),
            left_mean: a.posterior_mean,
            right_mean: b.posterior_mean,
            left_effective_trials: a.effective_trials,
            right_effective_trials: b.effective_trials,
            probability_left_exceeds_right: probability_of_superiority(&a, &b),
            naive_probability_left_exceeds_right: naive_probability_of_superiority(&a, &b),
            intervals_disjoint: a.separated_from(&b),
            caveat: COMPARISON_CAVEAT.to_string(),
        })
    }

    /// Everything the panel knows, including what it refuses to report and why.
    pub fn audit(&self) -> Result<PanelAudit, AdaptiveError> {
        let mut capabilities = Vec::new();
        let mut scored = 0usize;
        let mut abstentions = 0usize;
        for capability in self.ledger.capabilities() {
            let coverage = self.coverage(&capability);
            let stopping = self.stopping_verdict(&capability)?;
            scored += coverage.trials;
            abstentions += coverage.abstentions;
            let (estimate, withheld) = match self.estimate(&capability) {
                Ok(estimate) => (Some(estimate), None),
                Err(AdaptiveError::CoverageFloorNotMet { shortfalls, .. }) => {
                    (None, Some(shortfalls))
                }
                Err(other) => return Err(other),
            };
            capabilities.push(CapabilityAudit {
                cost: self.ledger.cost_of(&capability),
                capability,
                coverage,
                stopping,
                estimate,
                withheld,
            });
        }
        Ok(PanelAudit {
            trials: self.ledger.len(),
            scored_trials: scored,
            abstentions,
            total_cost: self.ledger.total_cost(),
            capabilities,
            caveat: AUDIT_CAVEAT.to_string(),
        })
    }
}

const COMPARISON_CAVEAT: &str = "Computed on independent clustered posteriors. Two architectures \
                                 scored on the same instances share instance-level noise and are \
                                 positively correlated, so this overstates separation for a \
                                 paired design; a paired estimator is not implemented.";

const AUDIT_CAVEAT: &str = "Effective trial counts assume one level of clustering (the parent \
                            world). Stopping decisions use a fixed credible interval checked \
                            repeatedly and are not anytime-valid, so a stopped interval's \
                            frequentist coverage is below its nominal level.";

/// A capability-versus-capability comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    pub left: CapabilityId,
    pub right: CapabilityId,
    pub left_mean: f64,
    pub right_mean: f64,
    pub left_effective_trials: f64,
    pub right_effective_trials: f64,
    /// `P(left > right)` on the clustered posteriors. The number to report.
    pub probability_left_exceeds_right: f64,
    /// The same quantity computed as if every instance were independent. Reported only so the
    /// gap between the two is visible.
    pub naive_probability_left_exceeds_right: f64,
    pub intervals_disjoint: bool,
    pub caveat: String,
}

impl Comparison {
    pub fn headline(&self) -> String {
        format!(
            "{} {:.3} vs {} {:.3}: P(left > right) = {:.3} on {:.1} and {:.1} effective trials. \
             Counting every instance as independent would have said {:.3}.",
            self.left,
            self.left_mean,
            self.right,
            self.right_mean,
            self.probability_left_exceeds_right,
            self.left_effective_trials,
            self.right_effective_trials,
            self.naive_probability_left_exceeds_right
        )
    }
}

/// One capability's entry in the audit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityAudit {
    pub capability: CapabilityId,
    /// What this capability's trials cost, abstentions included. 08.07 allocates budget per
    /// stratum, and a stratum that spent its budget on trials that could not be scored looks
    /// identical to one that spent nothing unless this is reported separately from the counts.
    pub cost: f64,
    pub coverage: CoverageStatus,
    pub stopping: StoppingVerdict,
    /// `None` when a coverage floor was not met.
    pub estimate: Option<CapabilityEstimate>,
    /// Why the estimate is absent, when it is.
    pub withheld: Option<String>,
}

/// The whole run, reportable and withheld parts alike.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelAudit {
    /// Trials recorded, abstentions included.
    pub trials: usize,
    /// Trials that became evidence.
    pub scored_trials: usize,
    pub abstentions: usize,
    pub total_cost: f64,
    pub capabilities: Vec<CapabilityAudit>,
    pub caveat: String,
}

impl PanelAudit {
    pub fn reported(&self) -> usize {
        self.capabilities
            .iter()
            .filter(|c| c.estimate.is_some())
            .count()
    }

    pub fn withheld(&self) -> usize {
        self.capabilities.len() - self.reported()
    }

    /// Total effective trials across reported capabilities.
    ///
    /// Folded from an explicit `0.0` rather than summed: `f64`'s additive identity in std is
    /// `-0.0`, and a report that opens with "rests on -0.0 effective trials" undermines the point
    /// it is making.
    pub fn effective_trials(&self) -> f64 {
        self.capabilities
            .iter()
            .filter_map(|c| c.estimate.as_ref())
            .fold(0.0, |total, estimate| total + estimate.effective_trials)
    }

    pub fn headline(&self) -> String {
        format!(
            "{} trials ({} scored, {} abstained) over {} capabilit(ies): {} reported, {} \
             withheld for coverage. The reported capabilities rest on {:.1} effective trials, \
             not {}.",
            self.trials,
            self.scored_trials,
            self.abstentions,
            self.capabilities.len(),
            self.reported(),
            self.withheld(),
            self.effective_trials(),
            self.scored_trials
        )
    }

    /// Content hash over canonical JSON, for referencing the audit from a run record.
    pub fn digest(&self) -> Result<ContentHash, AdaptiveError> {
        let value =
            serde_json::to_value(self).map_err(|e| AdaptiveError::Canonical(e.to_string()))?;
        ContentHash::of_value(&value).map_err(|e| AdaptiveError::Canonical(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{InstanceId, ParentId};
    use crate::ledger::Outcome;
    use crate::stopping::Question;

    fn panel() -> AdaptivePanel {
        AdaptivePanel::new(PanelConfig {
            coverage: CoveragePolicy {
                min_trials_per_capability: 20,
                min_parents_per_capability: 4,
                min_trials_per_parent: 2,
                max_parent_share: None,
                sentinels: Default::default(),
            },
            stopping: StoppingRule {
                budget: 500,
                question: Question::IntervalWidth { target: 0.15 },
                min_effective_trials: 8.0,
                credibility: 0.95,
            },
            bootstrap: Some(BootstrapConfig::default()),
            ..PanelConfig::default()
        })
    }

    /// Feeds one capability, one pass rate per parent.
    ///
    /// The rates differ deliberately: parents that behave identically produce an estimated `rho`
    /// of zero and no clustering correction at all, which is correct but tests nothing.
    fn feed(panel: &mut AdaptivePanel, capability: &str, rates: &[f64], per_parent: usize) {
        for (p, rate) in rates.iter().enumerate() {
            for i in 0..per_parent {
                let pass = (i as f64) < rate * per_parent as f64;
                panel
                    .record(
                        Trial::new(
                            CapabilityId::parse(capability).unwrap(),
                            InstanceId::parse(format!("{capability}-{p}-{i}")).unwrap(),
                            ParentId::parse(format!("{capability}-p{p}")).unwrap(),
                            if pass { Outcome::Pass } else { Outcome::Fail },
                            1.0,
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
        }
    }

    #[test]
    fn a_capability_below_the_coverage_floor_is_withheld_rather_than_reported() {
        let mut panel = panel();
        feed(&mut panel, "thin", &[0.5, 0.7], 3);
        let capability = CapabilityId::parse("thin").unwrap();
        assert!(matches!(
            panel.estimate(&capability),
            Err(AdaptiveError::CoverageFloorNotMet { .. })
        ));
        // The evidence itself is still available: the refusal is about the claim, not the data.
        assert_eq!(panel.summary(&capability).trials(), 6);

        let audit = panel.audit().unwrap();
        assert_eq!(audit.reported(), 0);
        assert_eq!(audit.withheld(), 1);
        assert!(audit.capabilities[0].withheld.is_some());
    }

    #[test]
    fn an_untouched_capability_is_an_unknown_capability_and_not_a_coverage_failure() {
        let panel = panel();
        assert!(matches!(
            panel.estimate(&CapabilityId::parse("never-run").unwrap()),
            Err(AdaptiveError::UnknownCapability { .. })
        ));
    }

    #[test]
    fn a_well_covered_capability_is_reported_with_both_intervals() {
        let mut panel = panel();
        feed(&mut panel, "cap", &[0.9, 0.8, 0.5, 0.7, 0.4, 0.8], 10);
        let estimate = panel.estimate(&CapabilityId::parse("cap").unwrap()).unwrap();
        assert_eq!(estimate.trials, 60);
        assert_eq!(estimate.parents, 6);
        assert!(estimate.clustered_interval.width() >= estimate.naive_interval.width());
        assert!(estimate.bootstrap_interval.is_some());
    }

    #[test]
    fn the_audit_digest_is_stable_across_recomputation_and_moves_with_the_evidence() {
        let mut panel = panel();
        feed(&mut panel, "cap", &[0.75, 0.5, 0.875, 0.375, 0.625], 8);
        let first = panel.audit().unwrap().digest().unwrap();
        let second = panel.audit().unwrap().digest().unwrap();
        assert_eq!(first, second);

        panel
            .record(
                Trial::new(
                    CapabilityId::parse("cap").unwrap(),
                    InstanceId::parse("extra").unwrap(),
                    ParentId::parse("cap-p9").unwrap(),
                    Outcome::Fail,
                    1.0,
                )
                .unwrap(),
            )
            .unwrap();
        assert_ne!(panel.audit().unwrap().digest().unwrap(), first);
    }

    #[test]
    fn the_audit_round_trips_through_json() {
        let mut panel = panel();
        feed(&mut panel, "a", &[0.875, 0.75, 1.0, 0.625, 0.875], 8);
        feed(&mut panel, "b", &[0.5, 0.0], 2);
        let audit = panel.audit().unwrap();
        let json = serde_json::to_string(&audit).unwrap();
        let back: PanelAudit = serde_json::from_str(&json).unwrap();

        // Compared field by field rather than with `assert_eq!`: serde_json's default float
        // parser is not correctly rounded, so a bit-exact round-trip of an f64-bearing struct is
        // not something this crate can promise without changing a workspace-wide feature. What
        // matters for an audit record is that the counts, the refusals and the interval bounds
        // survive, which is what is asserted.
        assert_eq!(back.trials, audit.trials);
        assert_eq!(back.scored_trials, audit.scored_trials);
        assert_eq!(back.reported(), 1);
        assert_eq!(back.withheld(), 1);
        for (got, want) in back.capabilities.iter().zip(&audit.capabilities) {
            assert_eq!(got.capability, want.capability);
            assert_eq!(got.coverage.shortfalls, want.coverage.shortfalls);
            assert_eq!(got.withheld, want.withheld);
            assert_eq!(got.stopping.reason, want.stopping.reason);
            if let (Some(got), Some(want)) = (&got.estimate, &want.estimate) {
                assert_eq!(got.trials, want.trials);
                assert!((got.effective_trials - want.effective_trials).abs() < 1e-9);
                assert!(
                    (got.clustered_interval.hi - want.clustered_interval.hi).abs() < 1e-12
                );
            }
        }
    }

    #[test]
    fn a_comparison_reports_the_clustered_and_the_naive_probability_side_by_side() {
        let mut panel = panel();
        feed(&mut panel, "strong", &[0.95, 0.9, 0.75, 0.95, 0.8, 0.85], 20);
        feed(&mut panel, "weak", &[0.5, 0.3, 0.45, 0.2, 0.4, 0.35], 20);
        let comparison = panel
            .compare(
                &CapabilityId::parse("strong").unwrap(),
                &CapabilityId::parse("weak").unwrap(),
            )
            .unwrap();
        assert!(comparison.probability_left_exceeds_right > 0.95);
        assert!(comparison.left_effective_trials < 120.0);
        assert!(comparison.headline().contains("effective trials"));
    }

    #[test]
    fn a_comparison_against_a_withheld_capability_refuses_rather_than_guessing() {
        let mut panel = panel();
        feed(&mut panel, "strong", &[0.95, 0.9, 0.75, 0.95, 0.8, 0.85], 20);
        feed(&mut panel, "thin", &[0.4], 3);
        assert!(matches!(
            panel.compare(
                &CapabilityId::parse("strong").unwrap(),
                &CapabilityId::parse("thin").unwrap()
            ),
            Err(AdaptiveError::CoverageFloorNotMet { .. })
        ));
    }

    #[test]
    fn the_panel_is_finished_only_once_every_touched_capability_has_stopped() {
        let mut panel = panel();
        assert!(
            !panel.finished().unwrap(),
            "an empty panel is not finished, it has not started"
        );
        feed(&mut panel, "a", &[0.9, 0.6, 0.8, 0.5, 0.7], 8);
        assert!(!panel.finished().unwrap());

        // A capability whose budget is spent stops, inconclusively.
        let mut spent = AdaptivePanel::new(PanelConfig {
            stopping: StoppingRule {
                budget: 40,
                question: Question::IntervalWidth { target: 0.001 },
                ..StoppingRule::default()
            },
            ..panel.config().clone()
        });
        feed(&mut spent, "a", &[0.9, 0.6, 0.8, 0.5, 0.7], 8);
        assert!(spent.finished().unwrap());
        let verdict = spent
            .stopping_verdict(&CapabilityId::parse("a").unwrap())
            .unwrap();
        assert!(!verdict.conclusive);
    }

    #[test]
    fn the_audit_reports_what_each_capability_cost() {
        let mut panel = panel();
        feed(&mut panel, "a", &[0.9, 0.6, 0.8, 0.5, 0.7], 8);
        feed(&mut panel, "b", &[0.5, 0.0], 2);
        let audit = panel.audit().unwrap();
        let costs: Vec<f64> = audit.capabilities.iter().map(|c| c.cost).collect();
        assert_eq!(costs, vec![40.0, 4.0]);
        assert_eq!(audit.total_cost, 44.0);
    }

    #[test]
    fn the_panel_config_round_trips_through_json() {
        let config = panel().config().clone();
        let json = serde_json::to_string(&config).unwrap();
        let back: PanelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, config);
    }
}
