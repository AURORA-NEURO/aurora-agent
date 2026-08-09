//! Budgets, resource accounting and cost.
//!
//! Blueprint 05.09 (budget, resource and cost controller). Two rules carry the module:
//!
//! - **An undeclared resource cannot be spent.** Not "spent freely" — refused. If a trial charges
//!   GPU seconds that its plan never budgeted, the comparison it was part of is already broken, and
//!   a zero-cost default would hide that.
//! - **Exhaustion aborts; it never truncates.** A refused charge leaves the meter exactly where it
//!   was and terminates the trial. The alternative — apply what fits, report what completed — turns
//!   a budget into a silent quality knob, and the resulting scores would say more about who ran out
//!   of tokens than about which architecture was better.
//!
//! Relationship to `bioprism-weave`'s `Budget`: not a duplicate, an opposite. Weave's budget is an
//! *affine* resource for coordination — it may be split among participants and may go unused, but
//! never copied, and the type has no `Clone` so the compiler enforces it. This one is a *meter* for
//! execution: it is read constantly, it distinguishes soft from hard limits, it accumulates
//! warnings, and it produces the comparable accounting a Pareto report needs. A single type would
//! have to be both non-copyable and freely observable, and would serve neither purpose well.
//!
//! Deliberately **not** implemented: price tables. 05.09 keeps estimated prices versioned and
//! separate from measured usage. `CostMicros` is therefore a resource like any other, charged by
//! whoever knows the price, and this module never multiplies tokens by a rate it invented.

use crate::error::RuntimeError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// What a trial can run out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeResource {
    ModelTokens,
    ModelCalls,
    ToolCalls,
    /// Real elapsed time. Kept for performance reporting.
    WallClockMillis,
    /// Virtual task time (05.07). Separate from wall clock on purpose: a task with a deadline must
    /// behave identically on a fast machine and a slow one.
    TaskTimeMillis,
    CpuMillis,
    MemoryBytes,
    StorageBytes,
    NetworkBytes,
    /// Monetary cost in micro-units, charged by the caller that knows the price.
    CostMicros,
}

impl fmt::Display for RuntimeResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            RuntimeResource::ModelTokens => "model_tokens",
            RuntimeResource::ModelCalls => "model_calls",
            RuntimeResource::ToolCalls => "tool_calls",
            RuntimeResource::WallClockMillis => "wall_clock_millis",
            RuntimeResource::TaskTimeMillis => "task_time_millis",
            RuntimeResource::CpuMillis => "cpu_millis",
            RuntimeResource::MemoryBytes => "memory_bytes",
            RuntimeResource::StorageBytes => "storage_bytes",
            RuntimeResource::NetworkBytes => "network_bytes",
            RuntimeResource::CostMicros => "cost_micros",
        };
        f.write_str(name)
    }
}

/// A soft warning line and a hard ceiling.
///
/// The soft limit exists so budget-aware policies can react — route to a cheaper architecture, stop
/// exploring — *before* the hard limit turns the trial into a failure. Which of the two a trial hit
/// is part of its evidence, so the distinction is kept rather than collapsed to "over budget".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limit {
    pub soft: Option<u64>,
    pub hard: u64,
}

impl Limit {
    pub fn hard(hard: u64) -> Self {
        Limit { soft: None, hard }
    }

    pub fn soft_then_hard(soft: u64, hard: u64) -> Self {
        Limit {
            soft: Some(soft),
            hard,
        }
    }
}

/// What a charge did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeStatus {
    /// Applied, still under the soft limit.
    Within,
    /// Applied, and this charge crossed the soft limit for the first time.
    SoftLimitCrossed,
    /// Applied, already past the soft limit.
    OverSoftLimit,
}

/// A soft-limit crossing, kept for the trial's accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetWarning {
    pub resource: RuntimeResource,
    pub soft: u64,
    pub used: u64,
}

/// The plan-level statement of what a trial may consume.
///
/// Serializable and freely copyable, unlike the controller it configures — a plan is a document
/// that travels, a meter is state that does not.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BudgetPlan {
    limits: BTreeMap<RuntimeResource, Limit>,
}

impl BudgetPlan {
    pub fn new() -> Self {
        BudgetPlan::default()
    }

    pub fn with(mut self, resource: RuntimeResource, limit: Limit) -> Self {
        self.limits.insert(resource, limit);
        self
    }

    pub fn limit(&self, resource: RuntimeResource) -> Option<Limit> {
        self.limits.get(&resource).copied()
    }

    pub fn resources(&self) -> impl Iterator<Item = RuntimeResource> + '_ {
        self.limits.keys().copied()
    }
}

/// The live meter for one trial.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetController {
    limits: BTreeMap<RuntimeResource, Limit>,
    used: BTreeMap<RuntimeResource, u64>,
    warnings: Vec<BudgetWarning>,
    aborted_on: Option<RuntimeResource>,
}

impl BudgetController {
    pub fn from_plan(plan: &BudgetPlan) -> Self {
        BudgetController {
            limits: plan.limits.clone(),
            used: BTreeMap::new(),
            warnings: Vec::new(),
            aborted_on: None,
        }
    }

    /// A meter that permits nothing, because nothing was declared.
    pub fn unfunded() -> Self {
        BudgetController::from_plan(&BudgetPlan::new())
    }

    pub fn used(&self, resource: RuntimeResource) -> u64 {
        self.used.get(&resource).copied().unwrap_or(0)
    }

    pub fn remaining(&self, resource: RuntimeResource) -> u64 {
        self.limits
            .get(&resource)
            .map_or(0, |limit| limit.hard.saturating_sub(self.used(resource)))
    }

    pub fn warnings(&self) -> &[BudgetWarning] {
        &self.warnings
    }

    /// The resource that ended the trial, if one did.
    pub fn aborted_on(&self) -> Option<RuntimeResource> {
        self.aborted_on
    }

    /// Charges usage, or refuses and aborts.
    ///
    /// On refusal `used` is unchanged. That is the invariant the whole module is built around: a
    /// charge is all-or-nothing, so the meter always reflects work that actually happened and never
    /// a partially applied allowance.
    pub fn charge(
        &mut self,
        resource: RuntimeResource,
        amount: u64,
    ) -> Result<ChargeStatus, RuntimeError> {
        if let Some(aborted) = self.aborted_on {
            return Err(RuntimeError::AlreadyAborted { resource: aborted });
        }
        let Some(limit) = self.limits.get(&resource).copied() else {
            return Err(RuntimeError::UndeclaredResource { resource });
        };

        let used = self.used(resource);
        let proposed = used.saturating_add(amount);
        if proposed > limit.hard {
            self.aborted_on = Some(resource);
            return Err(RuntimeError::BudgetExhausted {
                resource,
                hard: limit.hard,
                used,
                requested: amount,
            });
        }

        self.used.insert(resource, proposed);
        let Some(soft) = limit.soft else {
            return Ok(ChargeStatus::Within);
        };
        if proposed <= soft {
            Ok(ChargeStatus::Within)
        } else if used <= soft {
            self.warnings.push(BudgetWarning {
                resource,
                soft,
                used: proposed,
            });
            Ok(ChargeStatus::SoftLimitCrossed)
        } else {
            Ok(ChargeStatus::OverSoftLimit)
        }
    }

    /// Carves a child budget out of this one.
    ///
    /// 05.09's hierarchy is experiment → pack → cell → trial → component → provider, and a child
    /// may never exceed its parent's allocation. The parent is charged immediately for the whole
    /// child allocation, so two children cannot both be handed the same headroom.
    pub fn derive_child(&mut self, plan: &BudgetPlan) -> Result<BudgetController, RuntimeError> {
        for resource in plan.resources() {
            let requested = plan.limit(resource).expect("resource came from the plan").hard;
            let available = self.remaining(resource);
            if !self.limits.contains_key(&resource) {
                return Err(RuntimeError::UndeclaredResource { resource });
            }
            if requested > available {
                return Err(RuntimeError::OverAllocatedChild {
                    resource,
                    requested,
                    available,
                });
            }
        }
        for resource in plan.resources() {
            let requested = plan.limit(resource).expect("resource came from the plan").hard;
            self.charge(resource, requested)?;
        }
        Ok(BudgetController::from_plan(plan))
    }

    /// The comparable accounting a report needs: what was allowed, what was used.
    pub fn accounting(&self) -> BTreeMap<RuntimeResource, Accounting> {
        self.limits
            .iter()
            .map(|(resource, limit)| {
                (
                    *resource,
                    Accounting {
                        limit: *limit,
                        used: self.used(*resource),
                    },
                )
            })
            .collect()
    }
}

/// One resource's line in a trial's accounting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accounting {
    pub limit: Limit,
    pub used: u64,
}
