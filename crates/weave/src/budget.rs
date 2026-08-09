//! Affine budgets.
//!
//! Blueprint 23.16 and the kernel conformance list in 23.49: **budget non-duplication**. A budget
//! is an affine resource — it may be split and it may go unused, but it may never be copied. If a
//! participant could hand the same allowance to two sub-participants, the ceiling the operator set
//! would be meaningless.
//!
//! Splitting therefore *moves* the allowance out of the parent. There is no `clone` on a budget.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resource {
    Tokens,
    ToolCalls,
    WallClockMillis,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BudgetError {
    #[error("no budget for {0:?}")]
    Unallocated(Resource),

    #[error("{resource:?} exhausted: requested {requested}, {available} remaining")]
    Exhausted {
        resource: Resource,
        requested: u64,
        available: u64,
    },
}

/// A non-copyable allowance.
///
/// Deliberately does **not** derive `Clone`. Duplicating a budget is the failure this type exists
/// to make impossible, so the compiler enforces it rather than a runtime check.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Budget {
    allowances: BTreeMap<Resource, u64>,
    spent: BTreeMap<Resource, u64>,
}

impl Budget {
    pub fn new() -> Self {
        Budget {
            allowances: BTreeMap::new(),
            spent: BTreeMap::new(),
        }
    }

    pub fn with(mut self, resource: Resource, amount: u64) -> Self {
        *self.allowances.entry(resource).or_insert(0) += amount;
        self
    }

    pub fn remaining(&self, resource: Resource) -> u64 {
        let allowed = self.allowances.get(&resource).copied().unwrap_or(0);
        let used = self.spent.get(&resource).copied().unwrap_or(0);
        allowed.saturating_sub(used)
    }

    pub fn spend(&mut self, resource: Resource, amount: u64) -> Result<u64, BudgetError> {
        if !self.allowances.contains_key(&resource) {
            return Err(BudgetError::Unallocated(resource));
        }
        let available = self.remaining(resource);
        if amount > available {
            return Err(BudgetError::Exhausted {
                resource,
                requested: amount,
                available,
            });
        }
        *self.spent.entry(resource).or_insert(0) += amount;
        Ok(self.remaining(resource))
    }

    /// Moves an allowance out of this budget into a new one.
    ///
    /// The parent's remaining allowance drops by exactly what the child receives, so the sum
    /// across a delegation tree is conserved.
    pub fn split(&mut self, resource: Resource, amount: u64) -> Result<Budget, BudgetError> {
        self.spend(resource, amount)?;
        Ok(Budget::new().with(resource, amount))
    }

    /// Returns an unspent allowance to this budget.
    ///
    /// Only the *unspent* remainder returns. Whatever the child actually consumed stays consumed,
    /// which is what keeps the delegation tree's total conserved rather than merely restored.
    pub fn absorb(&mut self, child: Budget) {
        let resources: Vec<Resource> = child.allowances.keys().copied().collect();
        for resource in resources {
            let unused = child.remaining(resource);
            if unused > 0 {
                let used = self.spent.entry(resource).or_insert(0);
                *used = used.saturating_sub(unused);
            }
        }
    }

    pub fn total_remaining(&self) -> u64 {
        self.allowances
            .keys()
            .map(|resource| self.remaining(*resource))
            .sum()
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new()
    }
}
