//! The nonrenewable-resource ledger, and why a fork cannot spend an aliquot twice (26.06).
//!
//! 26.06's protocol step 2 is "enforce nonduplication across forks" and its first failure mode is
//! "fork duplicates the same aliquot". Both are about the same fact: BioPRISM replays a decision
//! cell down several branches, and a branch that consumes tissue has consumed *the* tissue. If
//! branch A and branch B each spend the last 40 µL of a pediatric biopsy, the comparison between
//! them is a comparison of two worlds that cannot both exist, and the resource figure attached to
//! each of them is a fiction.
//!
//! This is `bioprism-fiber`'s non-`Clone` `Budget` rule in a different currency, and it is a
//! different rule: a budget cannot be copied, whereas a specimen *can* be branched — you may
//! legitimately explore two uses of the same aliquot — but the two branches may then not be scored
//! as a joint plan. [`Ledger::fork`] therefore always succeeds and
//! [`Ledger::joint_feasibility`] is the thing that refuses.
//!
//! # Failure actions still cost
//!
//! 26.06's fifth failure mode is "resource cost omitted for failed actions". A sequencing run that
//! failed QC consumed the library all the same. [`Draw::outcome`] exists so that a failed draw is
//! recorded, and [`BranchLedger::consumed`] counts it; there is no path that skips a draw because
//! the action it funded did not work.
//!
//! # Units
//!
//! 26.06's fourth failure mode is "units are inconsistent". A [`Resource`] carries a unit string
//! and [`Ledger::draw`] refuses a draw quoted in a different one. There is no conversion table:
//! `bioprism-bioir` refuses the same way for the same reason — a factor this crate invented
//! between `mL` and `uL` would be indistinguishable from a factor the blueprint specified.
//! Quantities are integers in the resource's own unit, so no rounding can create or destroy
//! material.
//!
//! # Not implemented
//!
//! No cost model and no utility model. 26.06's metrics — "decision utility per specimen unit",
//! "cost-adjusted success", "resource-regret versus optimal policy" — all need a utility function
//! and an optimal policy that the section never defines. [`BranchLedger::residual`] reports what is
//! left, which 26.06 does ask for ("report residual resource value"), and the trade-off against
//! accuracy is left to [`crate::plane`], where it is at least visible as two dimensions rather than
//! buried in one number. No Pareto frontier: `bioprism-metrics` owns ranking, including the rule
//! that incomparable is a real answer.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::error::BurdenError;

/// What kind of thing is being spent.
///
/// The seven categories are 26.06's own "Evaluation target" list. The distinction that matters is
/// [`ResourceClass::is_nonrenewable`]: compute bought again tomorrow is not the same kind of loss
/// as the last viable cells from a resected tumour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClass {
    /// "tissue and aliquot consumption"
    TissueAliquot,
    /// "viable-cell depletion"
    ViableCells,
    /// "assay capacity"
    AssayCapacity,
    /// "expert review time"
    ExpertTime,
    /// "participant burden"
    ParticipantBurden,
    /// "privacy access"
    PrivacyAccess,
    /// "money, latency, and compute"
    ComputeAndMoney,
}

impl ResourceClass {
    /// Every class, in blueprint listing order.
    pub const ALL: [ResourceClass; 7] = [
        ResourceClass::TissueAliquot,
        ResourceClass::ViableCells,
        ResourceClass::AssayCapacity,
        ResourceClass::ExpertTime,
        ResourceClass::ParticipantBurden,
        ResourceClass::PrivacyAccess,
        ResourceClass::ComputeAndMoney,
    ];

    /// Whether spending this can be undone by spending money.
    ///
    /// Privacy access counts as nonrenewable: 26.06 lists "privacy exposure count" as a metric, and
    /// an exposure that happened cannot be unexposed.
    pub fn is_nonrenewable(self) -> bool {
        matches!(
            self,
            ResourceClass::TissueAliquot
                | ResourceClass::ViableCells
                | ResourceClass::ParticipantBurden
                | ResourceClass::PrivacyAccess
        )
    }
}

/// A declared pool of something spendable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub class: ResourceClass,
    /// Integer quantity in `unit`. Integers so that no rounding can conjure material.
    pub initial: u64,
    /// The unit this pool is quoted in. Compared by equality only; there is no conversion.
    pub unit: String,
}

impl Resource {
    /// Declare a pool.
    pub fn new(
        id: impl Into<String>,
        class: ResourceClass,
        initial: u64,
        unit: impl Into<String>,
    ) -> Self {
        Resource {
            id: id.into(),
            class,
            initial,
            unit: unit.into(),
        }
    }
}

/// Whether the action a draw funded worked.
///
/// Present so that a failed action's cost cannot be dropped, which is 26.06's fifth failure mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawOutcome {
    /// The action succeeded and the material bought a result.
    Productive,
    /// The action failed. The material is gone regardless.
    Wasted,
}

/// One withdrawal from one pool on one branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Draw {
    pub action: String,
    pub resource: String,
    pub amount: u64,
    pub unit: String,
    pub outcome: DrawOutcome,
    /// Whether this consumption destroys the material rather than borrowing it. 26.06 asks to
    /// "penalize avoidable destructive actions" and the ledger cannot tell destructive from
    /// nondestructive without being told.
    pub destructive: bool,
}

impl Draw {
    /// A productive, destructive draw — the common case for a consumed aliquot.
    pub fn spent(
        action: impl Into<String>,
        resource: impl Into<String>,
        amount: u64,
        unit: impl Into<String>,
    ) -> Self {
        Draw {
            action: action.into(),
            resource: resource.into(),
            amount,
            unit: unit.into(),
            outcome: DrawOutcome::Productive,
            destructive: true,
        }
    }

    /// Mark the funded action as having failed. The draw still counts.
    pub fn wasted(mut self) -> Self {
        self.outcome = DrawOutcome::Wasted;
        self
    }

    /// Mark the draw as non-destructive: the material survives the action.
    pub fn nondestructive(mut self) -> Self {
        self.destructive = false;
        self
    }
}

/// One branch's spending, plus what it inherited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchLedger {
    pub branch: String,
    /// The branch this one forked from, if any. A root branch has none.
    pub parent: Option<String>,
    draws: Vec<Draw>,
}

impl BranchLedger {
    /// Total drawn from one pool on this branch alone, successful and failed alike.
    pub fn consumed(&self, resource: &str) -> u64 {
        self.draws
            .iter()
            .filter(|d| d.resource == resource)
            .map(|d| d.amount)
            .sum()
    }

    /// Total drawn on this branch that funded an action which then failed.
    pub fn wasted(&self, resource: &str) -> u64 {
        self.draws
            .iter()
            .filter(|d| d.resource == resource && d.outcome == DrawOutcome::Wasted)
            .map(|d| d.amount)
            .sum()
    }

    /// The draws recorded on this branch, in order.
    pub fn draws(&self) -> &[Draw] {
        &self.draws
    }
}

/// The resource state of a whole fork tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    resources: BTreeMap<String, Resource>,
    branches: BTreeMap<String, BranchLedger>,
}

impl Ledger {
    /// An empty ledger with one root branch.
    pub fn new(root: impl Into<String>) -> Self {
        let root = root.into();
        let mut branches = BTreeMap::new();
        branches.insert(
            root.clone(),
            BranchLedger {
                branch: root,
                parent: None,
                draws: Vec::new(),
            },
        );
        Ledger {
            resources: BTreeMap::new(),
            branches,
        }
    }

    /// Declare a pool.
    pub fn declare(&mut self, resource: Resource) -> Result<(), BurdenError> {
        if self.resources.contains_key(&resource.id) {
            return Err(BurdenError::DuplicateResource(resource.id));
        }
        self.resources.insert(resource.id.clone(), resource);
        Ok(())
    }

    /// Branch from an existing branch. Always succeeds: exploring two uses of one aliquot is a
    /// legitimate thing to evaluate, and refusing here would prevent the very comparison 26.18
    /// is built on.
    pub fn fork(&mut self, parent: &str, child: impl Into<String>) -> Result<(), BurdenError> {
        if !self.branches.contains_key(parent) {
            return Err(BurdenError::UnknownResource(parent.to_string()));
        }
        let child = child.into();
        self.branches.insert(
            child.clone(),
            BranchLedger {
                branch: child,
                parent: Some(parent.to_string()),
                draws: Vec::new(),
            },
        );
        Ok(())
    }

    /// Record a withdrawal, refusing an overdraw against the branch's inherited remainder.
    ///
    /// "Inherited remainder" walks to the root: a child branch starts from whatever its ancestors
    /// had left, not from the pool's initial size.
    pub fn draw(&mut self, branch: &str, draw: Draw) -> Result<(), BurdenError> {
        let resource = self
            .resources
            .get(&draw.resource)
            .ok_or_else(|| BurdenError::UnknownResource(draw.resource.clone()))?;
        if resource.unit != draw.unit {
            return Err(BurdenError::UnitMismatch {
                resource: draw.resource.clone(),
                left: resource.unit.clone(),
                right: draw.unit.clone(),
            });
        }
        if !self.branches.contains_key(branch) {
            return Err(BurdenError::UnknownResource(branch.to_string()));
        }
        let remaining = self.remaining(branch, &draw.resource)?;
        if draw.amount > remaining {
            return Err(BurdenError::Overdraw {
                fork: branch.to_string(),
                resource: draw.resource.clone(),
                requested: draw.amount,
                remaining,
            });
        }
        self.branches
            .get_mut(branch)
            .expect("branch presence checked above")
            .draws
            .push(draw);
        Ok(())
    }

    /// What is left of `resource` on `branch`, after everything this branch and its ancestors spent.
    pub fn remaining(&self, branch: &str, resource: &str) -> Result<u64, BurdenError> {
        let pool = self
            .resources
            .get(resource)
            .ok_or_else(|| BurdenError::UnknownResource(resource.to_string()))?;
        let mut spent = 0u64;
        let mut cursor = Some(branch.to_string());
        while let Some(id) = cursor {
            let ledger = self
                .branches
                .get(&id)
                .ok_or(BurdenError::UnknownResource(id.clone()))?;
            spent = spent.saturating_add(ledger.consumed(resource));
            cursor = ledger.parent.clone();
        }
        Ok(pool.initial.saturating_sub(spent))
    }

    /// Whether a set of branches could all have happened.
    ///
    /// This is the check 26.06 step 2 asks for. Two sibling branches that each drew from a
    /// destructive nonrenewable pool describe mutually exclusive worlds, and reporting their
    /// resource use side by side as if both occurred is the "fork duplicates the same aliquot"
    /// failure. Renewable pools do not conflict: two branches each buying an hour of compute is
    /// two hours of compute, not a contradiction.
    pub fn joint_feasibility(&self, branches: &[&str]) -> Result<(), BurdenError> {
        let mut claimants: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for branch in branches {
            let ledger = self
                .branches
                .get(*branch)
                .ok_or_else(|| BurdenError::UnknownResource((*branch).to_string()))?;
            let mut touched = BTreeSet::new();
            for draw in &ledger.draws {
                if !draw.destructive {
                    continue;
                }
                let class = self
                    .resources
                    .get(&draw.resource)
                    .map(|r| r.class)
                    .ok_or_else(|| BurdenError::UnknownResource(draw.resource.clone()))?;
                if class.is_nonrenewable() && touched.insert(draw.resource.as_str()) {
                    claimants
                        .entry(draw.resource.as_str())
                        .or_default()
                        .push(branch);
                }
            }
        }
        for (resource, who) in claimants {
            if who.len() > 1 {
                return Err(BurdenError::ForkDoubleSpend {
                    fork: who[0].to_string(),
                    other: who[1].to_string(),
                    resource: resource.to_string(),
                });
            }
        }
        Ok(())
    }

    /// What every declared pool has left on `branch`.
    pub fn residual(&self, branch: &str) -> Result<BTreeMap<String, u64>, BurdenError> {
        let mut out = BTreeMap::new();
        for id in self.resources.keys() {
            out.insert(id.clone(), self.remaining(branch, id)?);
        }
        Ok(out)
    }

    /// Draws on `branch` that destroyed nonrenewable material for an action that then failed.
    ///
    /// 26.06's "avoidable tissue consumption" metric names a quantity it never defines — avoidable
    /// against what alternative? This reports the subset that is avoidable under the one reading
    /// that needs no counterfactual policy: material destroyed for nothing.
    pub fn wasted_nonrenewable(&self, branch: &str) -> Result<Vec<&Draw>, BurdenError> {
        let ledger = self
            .branches
            .get(branch)
            .ok_or_else(|| BurdenError::UnknownResource(branch.to_string()))?;
        let mut out = Vec::new();
        for draw in &ledger.draws {
            let class = self
                .resources
                .get(&draw.resource)
                .map(|r| r.class)
                .ok_or_else(|| BurdenError::UnknownResource(draw.resource.clone()))?;
            if draw.outcome == DrawOutcome::Wasted && draw.destructive && class.is_nonrenewable() {
                out.push(draw);
            }
        }
        Ok(out)
    }

    /// The ledger for one branch.
    pub fn branch(&self, branch: &str) -> Option<&BranchLedger> {
        self.branches.get(branch)
    }
}
