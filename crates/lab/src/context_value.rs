//! Context-value expansion: which piece of evidence to acquire next, and when to stop.
//!
//! Blueprint 09.02: *"Acquire the next piece of context based on expected decision value rather
//! than similarity alone or indiscriminate long-context loading."* Its stopping rule is four-way —
//! stop when the decision is robust across plausible hypotheses, when evidence obligations are
//! satisfied, when marginal value is low, or when the action should be deferred or escalated.
//!
//! # Value is declared, not estimated
//!
//! 09.02 defines value as *"expected reduction in harmful uncertainty, elimination of plausible
//! wrong actions, predicted downstream success gain, and risk reduction divided by token, latency,
//! monetary, and privacy cost"*. Three of those four numerators need a model of downstream success
//! that this workspace does not have, and inventing one would produce an ordering that looks
//! principled and is arbitrary.
//!
//! So [`AcquisitionAction::value`] is a number the caller writes down, and this module contributes
//! the parts that can be checked without a model:
//!
//! - an action must target at least one obligation that is actually on the frontier, so value is
//!   attached to a decision rather than to a document;
//! - an action that would discharge nothing outstanding is excluded with a reason;
//! - ordering is by declared value per unit cost, deterministic under ties;
//! - stopping is explicit and always names which of 09.02's four rules fired.
//!
//! # Privacy is not on the cost axis
//!
//! 09.02 lists privacy among the denominators, which invites dividing by it. This module does not.
//! An action that crosses a permission or privacy boundary is **excluded**, not discounted — there
//! is no exchange rate at which reading something you may not read becomes worth it, and a cost
//! model that offers one will eventually be handed a large enough numerator.
//!
//! # Not implemented, deliberately
//!
//! No execution — nothing here reads a file, runs a test or asks a user; the plan is the output.
//! No value learning and no feedback from outcome to declared value, so the ordering never
//! improves on its own. No monetary cost model. No clock: "latency units" are declared by the
//! caller in whatever unit they are willing to trade against a token, and this module only sums
//! them.

use crate::error::LabError;
use crate::hypothesis::SeparationVerdict;
use bioprism_obligation::{ObligationGraph, ObligationState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 09.02's action set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionKind {
    ReadRegion,
    SearchRepository,
    InspectLog,
    QueryDatabase,
    RetrievePaper,
    InspectMetadata,
    RunTest,
    AskUser,
    ConsultMemory,
    ObtainToolSchema,
}

/// Whether an action stays inside the decision's permission and privacy envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "boundary")]
pub enum PrivacyBoundary {
    Inside,
    /// Names the policy that forbids it. Excluded from every plan, at any value.
    Crosses { policy: String },
}

/// What an action costs, in units the caller declares.
///
/// `tokens` and `latency_units` are summed by [`AcquisitionCost::weight`] under an equivalence the
/// caller sets by choosing the latency unit. There is no privacy term: see the module docs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcquisitionCost {
    pub tokens: u64,
    pub latency_units: u64,
}

impl AcquisitionCost {
    pub fn new(tokens: u64, latency_units: u64) -> Self {
        AcquisitionCost {
            tokens,
            latency_units,
        }
    }

    pub fn weight(&self) -> u64 {
        self.tokens + self.latency_units
    }

    pub fn plus(self, other: AcquisitionCost) -> AcquisitionCost {
        AcquisitionCost {
            tokens: self.tokens + other.tokens,
            latency_units: self.latency_units + other.latency_units,
        }
    }

    pub fn fits_within(&self, budget: &AcquisitionCost) -> bool {
        self.tokens <= budget.tokens && self.latency_units <= budget.latency_units
    }
}

/// One candidate acquisition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcquisitionAction {
    pub id: String,
    pub kind: AcquisitionKind,
    /// Obligation ids this action would help discharge. Never empty.
    pub targets: Vec<String>,
    /// Declared decision value. Uncalibrated by construction; see the module docs.
    pub value: f64,
    pub cost: AcquisitionCost,
    pub privacy: PrivacyBoundary,
}

impl AcquisitionAction {
    pub fn new(id: impl Into<String>, kind: AcquisitionKind, value: f64) -> Self {
        AcquisitionAction {
            id: id.into(),
            kind,
            targets: Vec::new(),
            value,
            cost: AcquisitionCost::new(1, 0),
            privacy: PrivacyBoundary::Inside,
        }
    }

    pub fn targeting<I, S>(mut self, obligations: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.targets = obligations.into_iter().map(Into::into).collect();
        self
    }

    pub fn costing(mut self, tokens: u64, latency_units: u64) -> Self {
        self.cost = AcquisitionCost::new(tokens, latency_units);
        self
    }

    pub fn crossing(mut self, policy: impl Into<String>) -> Self {
        self.privacy = PrivacyBoundary::Crosses {
            policy: policy.into(),
        };
        self
    }

    /// Declared value per unit cost. The ordering key, and nothing more than a ratio of two
    /// numbers the caller supplied.
    pub fn value_per_unit_cost(&self) -> f64 {
        self.value / self.cost.weight() as f64
    }
}

/// Why an action did not make the plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "excluded_because")]
pub enum ExclusionReason {
    /// Crosses a privacy or permission boundary. Not a cost — a refusal.
    CrossesBoundary { policy: String },
    /// Every obligation it targets is already evidentially discharged.
    TargetsNothingOutstanding,
    /// Its value per unit cost is below the stated floor.
    BelowMarginalValueFloor { ratio: f64 },
    /// The remaining budget cannot pay for it.
    BudgetExhausted,
}

/// One action in the plan, with the ratio that put it there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlannedAcquisition {
    pub action: String,
    pub kind: AcquisitionKind,
    pub targets: Vec<String>,
    pub value_per_unit_cost: f64,
    pub cost: AcquisitionCost,
}

/// Which of 09.02's stopping rules fired.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "stopped_because")]
pub enum StopReason {
    /// Every obligation the actions target is evidentially discharged.
    ObligationsDischarged,
    /// Hypothesis separation already licenses one answer, so more context cannot change it.
    DecisionRobustAcrossHypotheses { surviving: String },
    /// The next action's value per unit cost is under the floor.
    MarginalValueBelowFloor { action: String, ratio: f64 },
    BudgetExhausted,
    /// Nothing admissible remains, but obligations are still outstanding. The decision is not
    /// ready and this is the case that should escalate rather than proceed.
    EvidenceUnreachable { outstanding: Vec<String> },
    /// The whole admissible set was planned and the budget still holds.
    AllActionsPlanned,
}

/// The ordered plan and everything left out of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expansion {
    pub ordered: Vec<PlannedAcquisition>,
    pub excluded: Vec<(String, ExclusionReason)>,
    pub spent: AcquisitionCost,
    pub stop: StopReason,
}

impl Expansion {
    /// Whether the plan leaves the decision short of evidence it needs.
    pub fn should_escalate(&self) -> bool {
        matches!(self.stop, StopReason::EvidenceUnreachable { .. })
    }
}

/// Orders acquisitions by declared value per unit cost and says why it stopped.
///
/// `separation` is optional and, when it licenses a single surviving hypothesis, short-circuits the
/// whole plan: 09.02's first stopping rule is that the decision is already robust, and spending a
/// budget to confirm a settled question is the long-context failure this module exists to avoid.
pub fn expand(
    actions: &[AcquisitionAction],
    graph: &ObligationGraph,
    budget: AcquisitionCost,
    marginal_value_floor: f64,
    separation: Option<&SeparationVerdict>,
) -> Result<Expansion, LabError> {
    let effective = graph
        .effective_states()
        .map_err(|error| LabError::Separation(crate::error::SeparationError::ObligationGraph(
            error.to_string(),
        )))?;

    for action in actions {
        if action.targets.is_empty() {
            return Err(LabError::ActionTargetsNoObligation(action.id.clone()));
        }
        if !action.value.is_finite() {
            return Err(LabError::NonFiniteValue {
                action: action.id.clone(),
                value: format!("{}", action.value),
            });
        }
        if action.cost.weight() == 0 {
            return Err(LabError::CostlessAction {
                action: action.id.clone(),
            });
        }
        for target in &action.targets {
            if !effective.contains_key(target) {
                return Err(LabError::ActionTargetsUnknownObligation {
                    action: action.id.clone(),
                    obligation: target.clone(),
                });
            }
        }
    }

    if let Some(SeparationVerdict::Separated { surviving, .. }) = separation {
        if surviving.len() == 1 {
            return Ok(Expansion {
                ordered: Vec::new(),
                excluded: Vec::new(),
                spent: AcquisitionCost::default(),
                stop: StopReason::DecisionRobustAcrossHypotheses {
                    surviving: surviving[0].clone(),
                },
            });
        }
    }

    let mut excluded: Vec<(String, ExclusionReason)> = Vec::new();
    let mut admissible: Vec<&AcquisitionAction> = Vec::new();
    for action in actions {
        if let PrivacyBoundary::Crosses { policy } = &action.privacy {
            excluded.push((
                action.id.clone(),
                ExclusionReason::CrossesBoundary {
                    policy: policy.clone(),
                },
            ));
            continue;
        }
        let outstanding = action.targets.iter().any(|target| {
            effective
                .get(target)
                .is_none_or(|state| !state.is_evidentially_discharged())
        });
        if outstanding {
            admissible.push(action);
        } else {
            excluded.push((
                action.id.clone(),
                ExclusionReason::TargetsNothingOutstanding,
            ));
        }
    }

    admissible.sort_by(|a, b| {
        b.value_per_unit_cost()
            .partial_cmp(&a.value_per_unit_cost())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    let outstanding_ids: Vec<String> = effective
        .iter()
        .filter(|(_, state)| !state.is_evidentially_discharged())
        .map(|(id, _)| id.clone())
        .collect();

    if admissible.is_empty() {
        let stop = if outstanding_ids.is_empty() {
            StopReason::ObligationsDischarged
        } else {
            StopReason::EvidenceUnreachable {
                outstanding: outstanding_ids,
            }
        };
        return Ok(Expansion {
            ordered: Vec::new(),
            excluded,
            spent: AcquisitionCost::default(),
            stop,
        });
    }

    let mut ordered = Vec::new();
    let mut spent = AcquisitionCost::default();
    let mut stop = StopReason::AllActionsPlanned;
    let mut planned_targets: BTreeSet<&str> = BTreeSet::new();

    for action in admissible {
        let ratio = action.value_per_unit_cost();
        if ratio < marginal_value_floor {
            excluded.push((
                action.id.clone(),
                ExclusionReason::BelowMarginalValueFloor { ratio },
            ));
            stop = StopReason::MarginalValueBelowFloor {
                action: action.id.clone(),
                ratio,
            };
            break;
        }
        let would_spend = spent.plus(action.cost);
        if !would_spend.fits_within(&budget) {
            excluded.push((action.id.clone(), ExclusionReason::BudgetExhausted));
            stop = StopReason::BudgetExhausted;
            break;
        }
        spent = would_spend;
        planned_targets.extend(action.targets.iter().map(String::as_str));
        ordered.push(PlannedAcquisition {
            action: action.id.clone(),
            kind: action.kind,
            targets: action.targets.clone(),
            value_per_unit_cost: ratio,
            cost: action.cost,
        });
    }

    if matches!(stop, StopReason::AllActionsPlanned) {
        let unreachable: Vec<String> = outstanding_ids
            .iter()
            .filter(|id| !planned_targets.contains(id.as_str()))
            .filter(|id| {
                effective
                    .get(*id)
                    .is_some_and(|state| *state != ObligationState::Unresolvable)
            })
            .cloned()
            .collect();
        if !unreachable.is_empty() {
            stop = StopReason::EvidenceUnreachable {
                outstanding: unreachable,
            };
        }
    }

    Ok(Expansion {
        ordered,
        excluded,
        spent,
        stop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_obligation::{Obligation, StateRecord};
    use bioprism_scope::Timestamp;

    const AT: Timestamp = Timestamp::from_nanos_utc(1_700_000_000_000_000_000);

    fn graph() -> ObligationGraph {
        let mut graph = ObligationGraph::new("decide whether to retry");
        graph
            .insert(Obligation::new("key_stable", "is the idempotency key stable?").mandatory())
            .unwrap();
        graph
            .insert(Obligation::new("clock_skew", "is the clock skewed?"))
            .unwrap();
        graph
    }

    fn discharged_graph() -> ObligationGraph {
        let mut graph = graph();
        for id in ["key_stable", "clock_skew"] {
            graph
                .record(
                    id,
                    StateRecord::new(ObligationState::Satisfied, "analyst", AT, 0.9)
                        .with_evidence(["doc://x"]),
                )
                .unwrap();
        }
        graph
    }

    fn budget() -> AcquisitionCost {
        AcquisitionCost::new(1_000, 100)
    }

    #[test]
    fn an_action_that_targets_no_obligation_is_refused() {
        let action = AcquisitionAction::new("a", AcquisitionKind::ReadRegion, 1.0);
        assert_eq!(
            expand(&[action], &graph(), budget(), 0.0, None),
            Err(LabError::ActionTargetsNoObligation("a".to_string()))
        );
    }

    #[test]
    fn an_action_targeting_an_obligation_the_graph_does_not_have_is_refused() {
        let action =
            AcquisitionAction::new("a", AcquisitionKind::ReadRegion, 1.0).targeting(["ghost"]);
        assert_eq!(
            expand(&[action], &graph(), budget(), 0.0, None),
            Err(LabError::ActionTargetsUnknownObligation {
                action: "a".to_string(),
                obligation: "ghost".to_string(),
            })
        );
    }

    #[test]
    fn a_costless_action_is_refused_rather_than_sorted_first_forever() {
        let action = AcquisitionAction::new("free", AcquisitionKind::ConsultMemory, 10.0)
            .targeting(["key_stable"])
            .costing(0, 0);
        assert_eq!(
            expand(&[action], &graph(), budget(), 0.0, None),
            Err(LabError::CostlessAction {
                action: "free".to_string()
            })
        );
    }

    #[test]
    fn actions_are_ordered_by_declared_value_per_unit_cost() {
        let actions = vec![
            AcquisitionAction::new("cheap-weak", AcquisitionKind::ConsultMemory, 1.0)
                .targeting(["key_stable"])
                .costing(10, 0),
            AcquisitionAction::new("dear-strong", AcquisitionKind::RunTest, 40.0)
                .targeting(["clock_skew"])
                .costing(100, 0),
        ];
        let plan = expand(&actions, &graph(), budget(), 0.0, None).unwrap();
        assert_eq!(
            plan.ordered
                .iter()
                .map(|entry| entry.action.as_str())
                .collect::<Vec<_>>(),
            vec!["dear-strong", "cheap-weak"]
        );
    }

    #[test]
    fn an_action_that_crosses_a_privacy_boundary_is_excluded_rather_than_discounted() {
        let actions = vec![
            AcquisitionAction::new("read-patient-notes", AcquisitionKind::ReadRegion, 1_000_000.0)
                .targeting(["key_stable"])
                .costing(1, 0)
                .crossing("consent:no-secondary-use"),
        ];
        let plan = expand(&actions, &graph(), budget(), 0.0, None).unwrap();
        assert!(plan.ordered.is_empty());
        assert_eq!(
            plan.excluded,
            vec![(
                "read-patient-notes".to_string(),
                ExclusionReason::CrossesBoundary {
                    policy: "consent:no-secondary-use".to_string()
                }
            )]
        );
    }

    #[test]
    fn expansion_stops_when_every_targeted_obligation_is_already_discharged() {
        let actions = vec![
            AcquisitionAction::new("a", AcquisitionKind::InspectLog, 5.0)
                .targeting(["key_stable", "clock_skew"])
                .costing(10, 1),
        ];
        let plan = expand(&actions, &discharged_graph(), budget(), 0.0, None).unwrap();
        assert_eq!(plan.stop, StopReason::ObligationsDischarged);
        assert!(plan.ordered.is_empty());
    }

    #[test]
    fn expansion_stops_immediately_when_separation_already_licenses_a_winner() {
        let verdict = SeparationVerdict::Separated {
            by: Vec::new(),
            retired: Vec::new(),
            surviving: vec!["retry".to_string()],
            remaining: Vec::new(),
        };
        let actions = vec![
            AcquisitionAction::new("a", AcquisitionKind::InspectLog, 5.0)
                .targeting(["key_stable"])
                .costing(10, 1),
        ];
        let plan = expand(&actions, &graph(), budget(), 0.0, Some(&verdict)).unwrap();
        assert_eq!(
            plan.stop,
            StopReason::DecisionRobustAcrossHypotheses {
                surviving: "retry".to_string()
            }
        );
        assert_eq!(plan.spent, AcquisitionCost::default());
    }

    #[test]
    fn expansion_stops_at_the_marginal_value_floor_and_names_the_action_it_declined() {
        let actions = vec![
            AcquisitionAction::new("strong", AcquisitionKind::RunTest, 10.0)
                .targeting(["key_stable"])
                .costing(10, 0),
            AcquisitionAction::new("weak", AcquisitionKind::SearchRepository, 0.1)
                .targeting(["clock_skew"])
                .costing(100, 0),
        ];
        let plan = expand(&actions, &graph(), budget(), 0.5, None).unwrap();
        assert_eq!(plan.ordered.len(), 1);
        assert!(matches!(
            plan.stop,
            StopReason::MarginalValueBelowFloor { ref action, .. } if action == "weak"
        ));
    }

    #[test]
    fn expansion_stops_at_the_budget_and_records_what_it_could_not_afford() {
        let actions = vec![
            AcquisitionAction::new("first", AcquisitionKind::RunTest, 10.0)
                .targeting(["key_stable"])
                .costing(80, 0),
            AcquisitionAction::new("second", AcquisitionKind::RunTest, 9.0)
                .targeting(["clock_skew"])
                .costing(80, 0),
        ];
        let plan = expand(&actions, &graph(), AcquisitionCost::new(100, 0), 0.0, None).unwrap();
        assert_eq!(plan.stop, StopReason::BudgetExhausted);
        assert_eq!(plan.spent, AcquisitionCost::new(80, 0));
        assert!(plan
            .excluded
            .contains(&("second".to_string(), ExclusionReason::BudgetExhausted)));
    }

    #[test]
    fn an_outstanding_obligation_no_action_reaches_makes_the_plan_escalate() {
        let actions = vec![
            AcquisitionAction::new("a", AcquisitionKind::InspectLog, 5.0)
                .targeting(["key_stable"])
                .costing(10, 1),
        ];
        let plan = expand(&actions, &graph(), budget(), 0.0, None).unwrap();
        assert!(plan.should_escalate());
        assert!(matches!(
            plan.stop,
            StopReason::EvidenceUnreachable { ref outstanding } if outstanding == &vec!["clock_skew".to_string()]
        ));
    }

    #[test]
    fn a_plan_that_covers_every_outstanding_obligation_does_not_escalate() {
        let actions = vec![
            AcquisitionAction::new("a", AcquisitionKind::InspectLog, 5.0)
                .targeting(["key_stable", "clock_skew"])
                .costing(10, 1),
        ];
        let plan = expand(&actions, &graph(), budget(), 0.0, None).unwrap();
        assert_eq!(plan.stop, StopReason::AllActionsPlanned);
        assert!(!plan.should_escalate());
    }
}
