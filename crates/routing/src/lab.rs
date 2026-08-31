//! The offline lab: observe every architecture on every task, then route as if you had not.
//!
//! Blueprint 09.01 separates an "offline lab" that "runs controlled ablations and architecture
//! search over benchmark panels" from a "runtime router" that "fingerprints a task and selects
//! among approved architecture configurations". Both live here, and the seam between them is the
//! only thing that makes the resulting number trustworthy.
//!
//! [`observe`] runs the whole approved panel on every task through `bioprism_baseline::compare`,
//! producing the complete outcome table. That table is what the oracle retrospective selector
//! reads. The router must never read it. [`run`] enforces the separation by rebuilding a
//! restricted [`EvidenceLedger`] for each task and calling
//! [`crate::policy::RoutingPolicy::route_unseen`], which errors rather than proceeds if the
//! restriction failed.
//!
//! ## Two holdouts, because they answer different questions
//!
//! [`Holdout::Task`] removes only the task being routed. This is the standard leave-one-out
//! protocol and it measures whether evidence about *similar* tasks transfers to a new instance —
//! which is what a deployment actually has.
//!
//! [`Holdout::Regime`] additionally removes every task sharing the routed task's structural
//! regime. The router must then extrapolate to a structure it has never seen. This is the harder
//! and more honest test of *conditioning*, and a router that only wins under [`Holdout::Task`]
//! has learned which architecture wins in a familiar regime, not how to read structure. Both are
//! worth reporting; the report records which one produced it.

use crate::architecture::{ApprovedSet, Architecture};
use crate::error::RoutingError;
use crate::evidence::{validate_task_id, EvidenceLedger, Observation};
use crate::fingerprint::{Fingerprint, Regime};
use crate::policy::RoutingPolicy;
use crate::report::{ComparatorPick, RoutingReport, TaskRow};
use bioprism_baseline::{compare, ContextStrategy};
use bioprism_fiber::Query;
use bioprism_world::World;
use serde::{Deserialize, Serialize};

/// One world plus one query: the unit a router is asked to choose an architecture for.
///
/// Not serialisable, because [`World`] deliberately retains the raw document it was parsed from
/// and a task is a reference to inputs rather than an artefact. The *outcomes* are the artefact,
/// and [`EvidenceLedger`] does serialise.
#[derive(Debug, Clone)]
pub struct Task {
    pub task_id: String,
    pub world: World,
    pub query: Query,
}

impl Task {
    pub fn new(
        task_id: impl Into<String>,
        world: World,
        query: Query,
    ) -> Result<Self, RoutingError> {
        let task_id = task_id.into();
        validate_task_id(&task_id)?;
        Ok(Task {
            task_id,
            world,
            query,
        })
    }

    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.world, &self.query)
    }

    pub fn regime(&self) -> Regime {
        self.fingerprint().regime()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Holdout {
    /// Leave one task out.
    Task,
    /// Leave the task and its whole structural regime out.
    Regime,
}

impl Holdout {
    pub fn as_str(self) -> &'static str {
        match self {
            Holdout::Task => "leave-one-task-out",
            Holdout::Regime => "leave-one-regime-out",
        }
    }

    fn caveat(self) -> String {
        match self {
            Holdout::Task => "Holdout is leave-one-task-out: structurally similar sibling tasks \
                              remain in the evidence, so this measures transfer to a new instance \
                              of a familiar structure, not extrapolation to an unfamiliar one."
                .to_string(),
            Holdout::Regime => "Holdout is leave-one-regime-out: every task sharing the routed \
                                task's structural regime was removed, so the router had to \
                                extrapolate to a structure it had never observed."
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabSettings {
    pub policy: RoutingPolicy,
    /// The architecture the fixed-default comparator uses on every task.
    pub fixed_default: Architecture,
    pub holdout: Holdout,
    pub calibration_bins: usize,
}

impl LabSettings {
    pub fn new(policy: RoutingPolicy, fixed_default: Architecture) -> Result<Self, RoutingError> {
        policy.approved.require(&fixed_default)?;
        policy.validate()?;
        Ok(LabSettings {
            policy,
            fixed_default,
            holdout: Holdout::Task,
            calibration_bins: 5,
        })
    }

    pub fn with_holdout(mut self, holdout: Holdout) -> Self {
        self.holdout = holdout;
        self
    }
}

/// Run every approved architecture on every task and record what happened.
///
/// This is the expensive, offline half. It is also the half that must never reach the router:
/// the ledger it returns contains each task's own answer, which is precisely what makes it usable
/// as the oracle bound and unusable as routing input.
///
/// An outcome the deterministic oracle would not judge is refused rather than recorded, in either
/// of the two shapes `bioprism_baseline::compare` reports it. The ledger's whole value is that
/// every row in it was measured; an unobserved outcome entered as `verdict_preserving: false`
/// would be indistinguishable from a demonstrated failure and would move the router's regret.
pub fn observe(tasks: &[Task], approved: &ApprovedSet) -> Result<EvidenceLedger, RoutingError> {
    if tasks.is_empty() {
        return Err(RoutingError::NoTasks);
    }

    let mut observations: Vec<Observation> = Vec::new();
    for task in tasks {
        validate_task_id(&task.task_id)?;
        let fingerprint = task.fingerprint();
        let owned: Vec<Box<dyn ContextStrategy>> =
            approved.iter().map(Architecture::strategy).collect();
        let borrowed: Vec<&dyn ContextStrategy> = owned.iter().map(AsRef::as_ref).collect();
        let comparison = compare(&task.world, &task.query, &borrowed).map_err(|error| {
            RoutingError::TaskNotComparable {
                task: task.task_id.clone(),
                detail: error.to_string(),
            }
        })?;

        for (architecture, result) in approved.iter().zip(comparison.results.iter()) {
            if result.name != architecture.label() {
                return Err(RoutingError::ArchitectureLabelMismatch {
                    architecture: format!("{architecture:?}"),
                    expected: architecture.label(),
                    actual: result.name.clone(),
                });
            }
            let Some(judgement) = result.judgement() else {
                let Some(refusal) = result.refusal() else {
                    return Err(RoutingError::UnjudgedObservation {
                        task: task.task_id.clone(),
                        architecture: architecture.label(),
                        detail: "comparison row had neither a judgement nor a refusal".into(),
                    });
                };
                return Err(RoutingError::UnjudgedObservation {
                    task: task.task_id.clone(),
                    architecture: architecture.label(),
                    detail: refusal.to_string(),
                });
            };
            observations.push(Observation {
                task_id: task.task_id.clone(),
                fingerprint: fingerprint.clone(),
                architecture: architecture.clone(),
                verdict_preserving: judgement.verdict_preserving,
                closure_complete: result.closure_complete(),
                status: judgement.status,
                facts_exposed: result.facts_exposed,
                total_facts: comparison.total_facts,
            });
        }
    }

    EvidenceLedger::new(observations)
}

/// Score all four Gate 6 comparators on every task, routing each one from restricted evidence.
pub fn run(tasks: &[Task], settings: &LabSettings) -> Result<RoutingReport, RoutingError> {
    settings.policy.validate()?;
    settings.policy.approved.require(&settings.fixed_default)?;

    let ledger = observe(tasks, &settings.policy.approved)?;
    let most_expensive = ledger
        .most_expensive_architecture()
        .ok_or(RoutingError::NoTasks)?;

    let mut rows: Vec<TaskRow> = Vec::new();
    for task in tasks {
        let fingerprint = task.fingerprint();
        let regime = fingerprint.regime();
        let visible = match settings.holdout {
            Holdout::Task => ledger.excluding_task(&task.task_id),
            Holdout::Regime => ledger.filtered(|observation| {
                !observation.task_id.eq_ignore_ascii_case(&task.task_id)
                    && observation.fingerprint.regime() != regime
            }),
        };

        let decision = settings
            .policy
            .route_unseen(&task.task_id, &fingerprint, &visible)?;

        let pick = |architecture: &Architecture| -> Result<ComparatorPick, RoutingError> {
            ledger
                .observation(&task.task_id, architecture)
                .map(ComparatorPick::of)
                .ok_or_else(|| RoutingError::IncompleteEvidence {
                    task: task.task_id.clone(),
                    architecture: architecture.label(),
                })
        };

        let oracle = ledger
            .best_for_task(&task.task_id)
            .map(ComparatorPick::of)
            .ok_or_else(|| RoutingError::IncompleteEvidence {
                task: task.task_id.clone(),
                architecture: "oracle".to_string(),
            })?;

        rows.push(TaskRow {
            task_id: task.task_id.clone(),
            regime,
            facts: task.world.facts.len(),
            abstained: decision.abstained,
            confidence: decision.confidence,
            reason: decision.reason,
            fixed_default: pick(&settings.fixed_default)?,
            most_expensive_default: pick(&most_expensive)?,
            router: pick(&decision.architecture)?,
            oracle,
        });
    }

    let mut report = RoutingReport::assemble(
        rows,
        settings.fixed_default.clone(),
        most_expensive,
        settings.calibration_bins,
    )?;
    report.caveats.push(settings.holdout.caveat());
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::spec_task;
    use bioprism_worldgen::WorldSpec;

    fn panel() -> ApprovedSet {
        ApprovedSet::new([
            Architecture::FullContext,
            Architecture::FiberCompiled,
            Architecture::LexicalTopK { k: 11 },
        ])
        .unwrap()
    }

    fn settings() -> LabSettings {
        LabSettings::new(
            RoutingPolicy::defaulting_to(panel(), Architecture::FullContext).unwrap(),
            Architecture::FiberCompiled,
        )
        .unwrap()
    }

    fn tasks() -> Vec<Task> {
        vec![
            spec_task("ref-24", &WorldSpec::reference_like(24)),
            spec_task("ref-48", &WorldSpec::reference_like(48)),
            spec_task("disc-24", &WorldSpec::discriminating(24)),
            spec_task("disc-48", &WorldSpec::discriminating(48)),
        ]
    }

    #[test]
    fn a_lab_over_zero_tasks_is_refused() {
        assert_eq!(observe(&[], &panel()).unwrap_err(), RoutingError::NoTasks);
        assert_eq!(run(&[], &settings()).unwrap_err(), RoutingError::NoTasks);
    }

    #[test]
    fn observing_records_one_row_per_task_and_architecture() {
        let tasks = tasks();
        let ledger = observe(&tasks, &panel()).unwrap();
        assert_eq!(ledger.len(), tasks.len() * panel().len());
        assert_eq!(ledger.task_ids().len(), tasks.len());
    }

    #[test]
    fn full_context_is_always_admissible_so_the_oracle_bound_is_never_negative() {
        let ledger = observe(&tasks(), &panel()).unwrap();
        for task_id in ledger.task_ids() {
            let best = ledger.best_for_task(task_id).unwrap();
            assert!(
                best.utility() >= 0.0,
                "task {task_id} has no admissible architecture, which cannot happen while full \
                 context is approved"
            );
        }
    }

    #[test]
    fn the_report_scores_every_task_under_both_holdouts() {
        for holdout in [Holdout::Task, Holdout::Regime] {
            let report = run(&tasks(), &settings().with_holdout(holdout)).unwrap();
            assert_eq!(report.tasks.len(), 4);
            assert_eq!(report.account.oracle.tasks, 4);
            assert!(report
                .caveats
                .iter()
                .any(|caveat| caveat.contains(match holdout {
                    Holdout::Task => "leave-one-task-out",
                    Holdout::Regime => "leave-one-regime-out",
                })));
        }
    }

    #[test]
    fn the_stricter_holdout_never_shows_the_router_more_evidence() {
        let tasks = tasks();
        let ledger = observe(&tasks, &panel()).unwrap();
        let task = &tasks[0];
        let regime = task.regime();

        let by_task = ledger.excluding_task(&task.task_id);
        let by_regime = ledger.filtered(|observation| {
            observation.task_id != task.task_id && observation.fingerprint.regime() != regime
        });
        assert!(by_regime.len() <= by_task.len());
        assert!(!by_regime.contains_task(&task.task_id));
    }

    #[test]
    fn a_fixed_default_outside_the_approved_set_is_refused() {
        let error = LabSettings::new(
            RoutingPolicy::defaulting_to(panel(), Architecture::FullContext).unwrap(),
            Architecture::QueryGraph,
        )
        .unwrap_err();
        assert_eq!(
            error,
            RoutingError::UnapprovedArchitecture {
                architecture: "query-graph".to_string()
            }
        );
    }

    #[test]
    fn a_task_needs_an_identifier() {
        let task = spec_task("named", &WorldSpec::reference_like(4));
        assert_eq!(
            Task::new("", task.world.clone(), task.query.clone()).unwrap_err(),
            RoutingError::EmptyTaskId
        );

        assert!(matches!(
            Task::new(" named", task.world.clone(), task.query.clone()).unwrap_err(),
            RoutingError::InvalidTaskId { .. }
        ));
        assert!(matches!(
            Task::new("bad\nname", task.world, task.query).unwrap_err(),
            RoutingError::InvalidTaskId { .. }
        ));
    }
}
