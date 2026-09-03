//! Release gates as executable predicates, not prose.
//!
//! Blueprint 33.01 ends with eight release gates, every one of them a sentence: "Missing and
//! abstained cases cannot disappear silently." "Aggregation does not count correlated descendants
//! as independent." "The score cannot be improved by violating a mandatory safety gate." A sentence
//! is not a gate. [`ReleaseGate::evaluate`] is: a pure predicate over a [`CapabilityGrid`] that
//! returns, for each check, whether it was met, which cell violated it, or why it could not be
//! evaluated at all.
//!
//! # Three outcomes, following `bioprism-lens`
//!
//! `bioprism-lens` distinguishes answered, refused and evidence-absent, and its gate keeps
//! "nobody ran the leakage lens" in a different variant from "the leakage lens found nothing". The
//! same distinction is the whole point here, because the commonest way a capability gate passes is
//! that the cell it guards was never measured. So:
//!
//! - [`GateOutcome::Met`] — the predicate held over evidence that existed.
//! - [`GateOutcome::Violated`] — the predicate failed, and the witness names the cell.
//! - [`GateOutcome::Unevaluable`] — there was nothing to check. An unmeasured cell, an absent cell,
//!   an interval ceiling over an estimate that has no interval.
//!
//! A gate with any violation is [`GateVerdict::Blocked`]. A gate with no violation but some
//! unevaluable predicate is [`GateVerdict::NotEvaluable`], never `Passed`. Only
//! [`GateVerdict::permits_release`] returns true, and only for `Passed`.
//!
//! # Noncompensatory by construction
//!
//! There is no gate score. [`GateReport`] has no aggregate field, no pass fraction and no weighting,
//! so there is no arithmetic in which a strong capability offsets a violated gate. That is 33.01's
//! "The score cannot be improved by violating a mandatory safety gate" enforced by the absence of
//! the operation rather than by a rule about it, and it is the same noncompensatory stance
//! `bioprism_atlas::CapabilityDimension` documents for safety.
//!
//! # Not implemented
//!
//! No safety, privacy or leakage *content*. 33.15's metrics — violation rate, identifier exposure,
//! approval bypass — need a policy engine and real transcripts; this module can only check that a
//! cell measuring them cleared a threshold, which is why every predicate here names a capability
//! rather than a policy. No signed bundles and no reproduction badges: 33.01's last two gates are
//! release-process artifacts, and `bioprism-atlas` says the same about them.

use crate::conditions::MeasurementConditions;
use crate::grid::CapabilityGrid;
use crate::interval::{ClusteringUnit, NoIntervalReason};
use bioprism_atlas::{CapabilityId, UnmeasuredReason};
use serde::{Deserialize, Serialize};

/// Why a predicate had nothing to decide on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "gap", rename_all = "snake_case")]
pub enum EvaluabilityGap {
    /// The cell exists and is a hole. Not a failure — an absence, and the reason says which kind.
    CellUnmeasured {
        capability: CapabilityId,
        reason: UnmeasuredReason,
    },
    /// The grid has no such capability at all, which is a different defect from a hole: the
    /// evaluation was scoped without it.
    CellAbsent { capability: CapabilityId },
    /// An uncertainty ceiling over an estimate that carries no interval. The commonest gap on a
    /// grid built from `bioprism-atlas`, which computes no intervals by design.
    NoInterval {
        capability: CapabilityId,
        reason: NoIntervalReason,
    },
    /// The grid itself failed structural validation, so no predicate may claim a result from it.
    /// This remains a gap rather than a pass, and therefore cannot permit release.
    InvalidGrid { detail: String },
    /// The grid has no measured cell at all, so a grid-wide predicate has no population.
    GridEmpty,
}

/// One predicate's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum GateOutcome {
    Met,
    /// The witness is a sentence naming the cell and the numbers, so a report never has to say
    /// "a gate failed" without saying which.
    Violated {
        witness: String,
    },
    Unevaluable {
        gap: EvaluabilityGap,
    },
}

impl GateOutcome {
    pub fn is_met(&self) -> bool {
        matches!(self, GateOutcome::Met)
    }

    pub fn is_violated(&self) -> bool {
        matches!(self, GateOutcome::Violated { .. })
    }

    pub fn is_unevaluable(&self) -> bool {
        matches!(self, GateOutcome::Unevaluable { .. })
    }
}

/// One checkable release condition.
///
/// Every variant names either a capability or the whole grid, because a gate that cannot say what
/// it checked cannot be acted on. Thresholds are the caller's: this crate has no view on what a
/// good pass rate is, and inventing one would be the universal score in miniature.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "predicate", rename_all = "snake_case")]
pub enum GatePredicate {
    /// The named capability scores at least `floor`, in the grid's declared direction.
    MinimumScore {
        capability: CapabilityId,
        floor: f64,
    },
    /// Every measured cell clears `floor`. 33.01's "worst-domain behavior" as a gate.
    WorstCellAtLeast { floor: f64 },
    /// At least `floor` of the grid's in-scope cells are measured. This is the gate that stops a
    /// release resting on four measured capabilities out of forty.
    MinimumCoverage { floor: f64 },
    /// The named capability rests on at least `floor` independent clustering units — 33.01's
    /// "Aggregation does not count correlated descendants as independent", and 33.19's worked
    /// example of a million instances from three hundred parent worlds.
    MinimumEffectiveSize {
        capability: CapabilityId,
        floor: usize,
    },
    /// 33.01's uncertainty ceiling. **Unevaluable, not met**, when the estimate has no interval:
    /// an unbounded width is not a narrow one.
    MaximumIntervalWidth {
        capability: CapabilityId,
        ceiling: f64,
    },
    /// The named capability's interval clusters at a dependency level rather than at the trial.
    /// 33.01: "Confidence intervals cluster at the highest dependency level."
    IntervalClustersAboveTrial { capability: CapabilityId },
    /// None of the named capabilities is a hole. Distinct from a score floor: this gate is about
    /// whether anyone looked.
    NoUnmeasured { capabilities: Vec<CapabilityId> },
    /// A named condition coordinate was recorded. Always evaluable — the question is whether the
    /// label exists, and a missing label is a violation rather than a gap.
    ConditionRecorded { dimension: String },
}

impl GatePredicate {
    pub fn kind(&self) -> &'static str {
        match self {
            GatePredicate::MinimumScore { .. } => "minimum_score",
            GatePredicate::WorstCellAtLeast { .. } => "worst_cell_at_least",
            GatePredicate::MinimumCoverage { .. } => "minimum_coverage",
            GatePredicate::MinimumEffectiveSize { .. } => "minimum_effective_size",
            GatePredicate::MaximumIntervalWidth { .. } => "maximum_interval_width",
            GatePredicate::IntervalClustersAboveTrial { .. } => "interval_clusters_above_trial",
            GatePredicate::NoUnmeasured { .. } => "no_unmeasured",
            GatePredicate::ConditionRecorded { .. } => "condition_recorded",
        }
    }

    fn invalid_reason(&self) -> Option<String> {
        match self {
            GatePredicate::MinimumScore { floor, .. } if !floor.is_finite() => {
                Some(format!("minimum score floor {floor} is not finite"))
            }
            GatePredicate::WorstCellAtLeast { floor } if !floor.is_finite() => {
                Some(format!("worst-cell floor {floor} is not finite"))
            }
            GatePredicate::MinimumCoverage { floor } if !floor.is_finite() => {
                Some(format!("coverage floor {floor} is not finite"))
            }
            GatePredicate::MinimumCoverage { floor } if !(0.0..=1.0).contains(floor) => {
                Some(format!("coverage floor {floor} is outside 0..=1"))
            }
            GatePredicate::MaximumIntervalWidth { ceiling, .. } if !ceiling.is_finite() => {
                Some(format!("interval-width ceiling {ceiling} is not finite"))
            }
            GatePredicate::MaximumIntervalWidth { ceiling, .. } if *ceiling < 0.0 => {
                Some(format!("interval-width ceiling {ceiling} is negative"))
            }
            GatePredicate::NoUnmeasured { capabilities } if capabilities.is_empty() => {
                Some("no-unmeasured must name at least one capability".to_string())
            }
            _ => None,
        }
    }

    /// Evaluates one predicate against one grid.
    pub fn evaluate(&self, grid: &CapabilityGrid) -> GateOutcome {
        if let Some(reason) = self.invalid_reason() {
            return GateOutcome::Violated {
                witness: format!("invalid {} predicate: {reason}", self.kind()),
            };
        }
        if let Err(error) = grid.validate() {
            return GateOutcome::Unevaluable {
                gap: EvaluabilityGap::InvalidGrid {
                    detail: error.to_string(),
                },
            };
        }
        match self {
            GatePredicate::MinimumScore { capability, floor } => match resolve(grid, capability) {
                Err(gap) => GateOutcome::Unevaluable { gap },
                Ok(value) => {
                    let direction = grid.conditions.scoring_rule.direction;
                    if direction.is_better(*floor, value) {
                        GateOutcome::Violated {
                            witness: format!(
                                "{capability} scores {value} against a floor of {floor} \
                                     ({})",
                                direction.as_str()
                            ),
                        }
                    } else {
                        GateOutcome::Met
                    }
                }
            },
            GatePredicate::WorstCellAtLeast { floor } => {
                let direction = grid.conditions.scoring_rule.direction;
                let mut worst: Option<(&CapabilityId, f64)> = None;
                for (capability, cell) in grid.measured() {
                    if let Some(value) = cell.value() {
                        if worst.is_none_or(|(_, current)| direction.is_better(current, value)) {
                            worst = Some((capability, value));
                        }
                    }
                }
                match worst {
                    None => GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::GridEmpty,
                    },
                    Some((capability, value)) if direction.is_better(*floor, value) => {
                        GateOutcome::Violated {
                            witness: format!(
                                "worst measured cell {capability} scores {value} against a floor \
                                 of {floor}"
                            ),
                        }
                    }
                    Some(_) => GateOutcome::Met,
                }
            }
            GatePredicate::MinimumCoverage { floor } => {
                let in_scope = grid
                    .cells()
                    .filter(|(_, cell)| cell.is_measured() || !cell.hole_is_closed_by_declaration())
                    .count();
                if in_scope == 0 {
                    return GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::GridEmpty,
                    };
                }
                let measured = grid.measured().count();
                let coverage = measured as f64 / in_scope as f64;
                if coverage < *floor {
                    GateOutcome::Violated {
                        witness: format!(
                            "{measured} of {in_scope} in-scope cells measured, coverage \
                             {coverage:.4} against a floor of {floor}"
                        ),
                    }
                } else {
                    GateOutcome::Met
                }
            }
            GatePredicate::MinimumEffectiveSize { capability, floor } => {
                let Some(cell) = grid.cell(capability) else {
                    return GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::CellAbsent {
                            capability: capability.clone(),
                        },
                    };
                };
                match cell.effective_size() {
                    None => GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::CellUnmeasured {
                            capability: capability.clone(),
                            reason: cell
                                .unmeasured_reason()
                                .unwrap_or(UnmeasuredReason::NotAttempted),
                        },
                    },
                    Some(size) if size < *floor => GateOutcome::Violated {
                        witness: format!(
                            "{capability} rests on {size} independent units against a floor of \
                             {floor}"
                        ),
                    },
                    Some(_) => GateOutcome::Met,
                }
            }
            GatePredicate::MaximumIntervalWidth {
                capability,
                ceiling,
            } => {
                let Some(cell) = grid.cell(capability) else {
                    return GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::CellAbsent {
                            capability: capability.clone(),
                        },
                    };
                };
                let Some(estimate) = cell.estimate() else {
                    return GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::CellUnmeasured {
                            capability: capability.clone(),
                            reason: cell
                                .unmeasured_reason()
                                .unwrap_or(UnmeasuredReason::NotAttempted),
                        },
                    };
                };
                match estimate.interval() {
                    None => GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::NoInterval {
                            capability: capability.clone(),
                            reason: estimate
                                .no_interval_reason()
                                .unwrap_or(NoIntervalReason::EstimatorNotAvailable),
                        },
                    },
                    Some(interval) if interval.width() > *ceiling => GateOutcome::Violated {
                        witness: format!(
                            "{capability} has interval width {} against a ceiling of {ceiling}",
                            interval.width()
                        ),
                    },
                    Some(_) => GateOutcome::Met,
                }
            }
            GatePredicate::IntervalClustersAboveTrial { capability } => {
                let Some(cell) = grid.cell(capability) else {
                    return GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::CellAbsent {
                            capability: capability.clone(),
                        },
                    };
                };
                let Some(estimate) = cell.estimate() else {
                    return GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::CellUnmeasured {
                            capability: capability.clone(),
                            reason: cell
                                .unmeasured_reason()
                                .unwrap_or(UnmeasuredReason::NotAttempted),
                        },
                    };
                };
                match estimate.interval() {
                    None => GateOutcome::Unevaluable {
                        gap: EvaluabilityGap::NoInterval {
                            capability: capability.clone(),
                            reason: estimate
                                .no_interval_reason()
                                .unwrap_or(NoIntervalReason::EstimatorNotAvailable),
                        },
                    },
                    Some(interval) => {
                        let unit = interval.basis().clustering_unit;
                        if unit.is_dependency_level() {
                            GateOutcome::Met
                        } else {
                            GateOutcome::Violated {
                                witness: format!(
                                    "{capability} reports an interval clustered at {}, which \
                                     counts correlated descendants as independent",
                                    ClusteringUnit::Trial
                                ),
                            }
                        }
                    }
                }
            }
            GatePredicate::NoUnmeasured { capabilities } => {
                for capability in capabilities {
                    match grid.cell(capability) {
                        None => {
                            return GateOutcome::Violated {
                                witness: format!("{capability} is absent from the grid"),
                            }
                        }
                        Some(cell) => {
                            if let Some(reason) = cell.unmeasured_reason() {
                                return GateOutcome::Violated {
                                    witness: format!("{capability} is unmeasured ({reason})"),
                                };
                            }
                        }
                    }
                }
                GateOutcome::Met
            }
            GatePredicate::ConditionRecorded { dimension } => {
                if condition_recorded(&grid.conditions, dimension) {
                    GateOutcome::Met
                } else {
                    GateOutcome::Violated {
                        witness: format!(
                            "condition coordinate {dimension} is unrecorded, so no comparison \
                             against another grid can establish it matched"
                        ),
                    }
                }
            }
        }
    }
}

/// One predicate paired with its outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PredicateOutcome {
    pub predicate: GatePredicate,
    pub outcome: GateOutcome,
}

/// Whether a release may proceed.
///
/// Three states, and the middle one is why the enum exists. Precedence is: any violation gives
/// `Blocked`, because a real failure is decisive and hiding it behind an unevaluable predicate
/// would be generous in the wrong direction; otherwise any gap gives `NotEvaluable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    Passed,
    Blocked,
    /// Nothing failed, and nothing was checked either. Never `Passed`: 33.01's whole first module
    /// is about the difference.
    NotEvaluable,
}

impl GateVerdict {
    /// The only true answer to "may this ship". `NotEvaluable` returns false.
    pub fn permits_release(self) -> bool {
        matches!(self, GateVerdict::Passed)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            GateVerdict::Passed => "passed",
            GateVerdict::Blocked => "blocked",
            GateVerdict::NotEvaluable => "not_evaluable",
        }
    }
}

/// A named set of predicates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseGate {
    pub name: String,
    pub predicates: Vec<GatePredicate>,
}

impl ReleaseGate {
    pub fn new(name: impl Into<String>) -> Self {
        ReleaseGate {
            name: name.into(),
            predicates: Vec::new(),
        }
    }

    pub fn requiring(mut self, predicate: GatePredicate) -> Self {
        self.predicates.push(predicate);
        self
    }

    /// Evaluates every predicate. All of them, always: a gate that short-circuits on the first
    /// violation reports one problem when there are five, and a release team then fixes them one
    /// release at a time.
    pub fn evaluate(&self, grid: &CapabilityGrid) -> GateReport {
        let outcomes: Vec<PredicateOutcome> = self
            .predicates
            .iter()
            .map(|predicate| PredicateOutcome {
                predicate: predicate.clone(),
                outcome: predicate.evaluate(grid),
            })
            .collect();
        let verdict = if outcomes.iter().any(|o| o.outcome.is_violated()) {
            GateVerdict::Blocked
        } else if outcomes.iter().any(|o| o.outcome.is_unevaluable()) {
            GateVerdict::NotEvaluable
        } else {
            GateVerdict::Passed
        };
        GateReport {
            gate: self.name.clone(),
            grid: grid.label.clone(),
            verdict,
            outcomes,
        }
    }
}

/// The result of a gate over one grid.
///
/// Carries no score, no pass fraction and no weighting, so there is no arithmetic by which a strong
/// capability offsets a violated gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateReport {
    pub gate: String,
    pub grid: String,
    pub verdict: GateVerdict,
    /// Every predicate, including the ones that passed. A gate report listing only failures cannot
    /// distinguish "we checked eleven things and one failed" from "we checked one thing".
    pub outcomes: Vec<PredicateOutcome>,
}

impl GateReport {
    pub fn violations(&self) -> Vec<&PredicateOutcome> {
        self.outcomes
            .iter()
            .filter(|o| o.outcome.is_violated())
            .collect()
    }

    /// The predicates that had nothing to check. The list a release team acts on before
    /// re-running the gate.
    pub fn gaps(&self) -> Vec<&PredicateOutcome> {
        self.outcomes
            .iter()
            .filter(|o| o.outcome.is_unevaluable())
            .collect()
    }

    pub fn headline(&self) -> String {
        format!(
            "gate {} over {}: {} ({} predicates, {} violated, {} unevaluable)",
            self.gate,
            self.grid,
            self.verdict.as_str(),
            self.outcomes.len(),
            self.violations().len(),
            self.gaps().len()
        )
    }
}

fn resolve(grid: &CapabilityGrid, capability: &CapabilityId) -> Result<f64, EvaluabilityGap> {
    let Some(cell) = grid.cell(capability) else {
        return Err(EvaluabilityGap::CellAbsent {
            capability: capability.clone(),
        });
    };
    cell.value().ok_or_else(|| EvaluabilityGap::CellUnmeasured {
        capability: capability.clone(),
        reason: cell
            .unmeasured_reason()
            .unwrap_or(UnmeasuredReason::NotAttempted),
    })
}

fn condition_recorded(conditions: &MeasurementConditions, dimension: &str) -> bool {
    match dimension {
        "ontology version" => conditions.ontology_version.is_recorded(),
        "pack version" => conditions.pack_version.is_recorded(),
        "evidence base" => conditions.evidence_base.is_recorded(),
        "oracle floor" | "oracle tier" => conditions.oracle_floor.is_recorded(),
        "budget" => conditions.budget.is_recorded(),
        other => conditions.stratum.get(other).is_recorded(),
    }
}
