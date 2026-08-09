//! The affine tissue ledger.
//!
//! Blueprint 24.10: "Every destructive or depleting action posts to an affine ledger: consumed
//! resources cannot be duplicated by branching. Forked simulations may inspect hypothetical
//! outcomes, but only one material branch can consume the real-world allocation."
//!
//! Affine is a type-system word, so this module takes it literally.
//! [`TissueLedger::fork_material`] takes `self` **by value**. The parent ledger is moved into
//! the branch and ceases to exist, so a second material fork is not a runtime error to be
//! caught by a validator — it does not compile. A benchmark cannot accidentally give two
//! architectures the same last aliquot, because there is no way to write it.
//!
//! [`TissueLedger::fork_hypothetical`] borrows instead, and hypothetical branches may inspect
//! anything but consume no material. That asymmetry is the whole rule.
//!
//! The module also implements 24.10's utility expression
//! `U(a) = E[ΔD(a)] − λc·C(a) − λt·T(a) − λs·S(a) − λp·P(a)` in
//! [`UtilityWeights::utility`]. The formula is the blueprint's; **the weights are not**. Section
//! 24 gives no values for λ, and there is no defensible way to derive them from the text, so
//! [`UtilityWeights::default`] is a stated placeholder rather than a recommendation. Its only
//! real guarantee is structural: with any positive λs, an action that consumes specimen and
//! gains nothing scores below abstention, which is the non-negotiable rule at the end of 24.10.

use crate::error::LedgerError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The depletable resource types of blueprint 24.10.
///
/// The blueprint's tenth bullet — "opportunity cost of consuming a nonrenewable specimen" — is
/// not a kind here, because it is not a balance anyone holds. It is what
/// [`ResourceKind::is_nonrenewable_material`] being true *means*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    /// Tissue mass or section count.
    TissueMass,
    ViableCells,
    /// DNA, RNA, protein or metabolite aliquots.
    Aliquot,
    ImagingOrSequencingCapacity,
    LaboratoryOrExpertTime,
    ParticipantBurden,
    PrivacyExposure,
    MonetaryCost,
    TurnaroundTime,
}

impl ResourceKind {
    pub const ALL: [ResourceKind; 9] = [
        ResourceKind::TissueMass,
        ResourceKind::ViableCells,
        ResourceKind::Aliquot,
        ResourceKind::ImagingOrSequencingCapacity,
        ResourceKind::LaboratoryOrExpertTime,
        ResourceKind::ParticipantBurden,
        ResourceKind::PrivacyExposure,
        ResourceKind::MonetaryCost,
        ResourceKind::TurnaroundTime,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ResourceKind::TissueMass => "tissue mass",
            ResourceKind::ViableCells => "viable cells",
            ResourceKind::Aliquot => "aliquot",
            ResourceKind::ImagingOrSequencingCapacity => "imaging or sequencing capacity",
            ResourceKind::LaboratoryOrExpertTime => "laboratory or expert time",
            ResourceKind::ParticipantBurden => "participant burden",
            ResourceKind::PrivacyExposure => "privacy exposure",
            ResourceKind::MonetaryCost => "monetary cost",
            ResourceKind::TurnaroundTime => "turnaround time",
        }
    }

    /// Whether spending this can never be undone or re-acquired.
    ///
    /// Capacity, time and money are scarce but replenishable; a consumed aliquot and an
    /// exposed participant are not. Only the nonrenewable kinds are barred from hypothetical
    /// branches, because simulating a compute spend costs nobody a specimen.
    pub fn is_nonrenewable_material(self) -> bool {
        matches!(
            self,
            ResourceKind::TissueMass
                | ResourceKind::ViableCells
                | ResourceKind::Aliquot
                | ResourceKind::ParticipantBurden
                | ResourceKind::PrivacyExposure
        )
    }
}

/// Which kind of branch a ledger belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchMode {
    /// Holds the real-world allocation. Exactly one of these exists per allocation, enforced by
    /// [`TissueLedger::fork_material`] consuming its receiver.
    Material,
    /// May inspect, may spend renewable budget, may never consume specimen.
    Hypothetical,
}

/// Identity of one depletable holding.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceId {
    pub kind: ResourceKind,
    /// Which specific holding: a block, an aliquot tube, a participant, a budget line.
    pub name: String,
}

impl ResourceId {
    pub fn new(kind: ResourceKind, name: impl Into<String>) -> Self {
        ResourceId {
            kind,
            name: name.into(),
        }
    }

    fn describe(&self) -> String {
        format!("{} ({})", self.name, self.kind.as_str())
    }
}

/// A non-duplicable account of scarce material.
///
/// Does not implement `Clone`, matching the workspace rule that a budget cannot be duplicated.
/// `fork_material` moving its receiver would be worth little if `ledger.clone()` handed out a
/// second copy of the same last aliquot.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TissueLedger {
    mode: BranchMode,
    balances: BTreeMap<ResourceId, f64>,
    spent: BTreeMap<ResourceId, f64>,
}

impl TissueLedger {
    /// A new ledger holding the real-world allocation.
    pub fn material() -> Self {
        TissueLedger {
            mode: BranchMode::Material,
            balances: BTreeMap::new(),
            spent: BTreeMap::new(),
        }
    }

    pub fn mode(&self) -> BranchMode {
        self.mode
    }

    pub fn stock(mut self, resource: ResourceId, quantity: f64) -> Result<Self, LedgerError> {
        if !quantity.is_finite() || quantity < 0.0 {
            return Err(LedgerError::InvalidQuantity { value: quantity });
        }
        *self.balances.entry(resource).or_insert(0.0) += quantity;
        Ok(self)
    }

    pub fn available(&self, resource: &ResourceId) -> f64 {
        self.balances.get(resource).copied().unwrap_or(0.0)
    }

    pub fn spent(&self, resource: &ResourceId) -> f64 {
        self.spent.get(resource).copied().unwrap_or(0.0)
    }

    /// Posts a depleting action.
    ///
    /// Refuses three ways: an unusable quantity, a hypothetical branch reaching for real
    /// material, and an overdraft. The overdraft message reports what was left, because "you
    /// asked for two sections and one remains" is a fact an agent can act on and "insufficient
    /// resources" is not.
    pub fn consume(&mut self, resource: &ResourceId, quantity: f64) -> Result<(), LedgerError> {
        if !quantity.is_finite() || quantity < 0.0 {
            return Err(LedgerError::InvalidQuantity { value: quantity });
        }
        if self.mode == BranchMode::Hypothetical && resource.kind.is_nonrenewable_material() {
            return Err(LedgerError::HypotheticalConsumption {
                resource: resource.describe(),
            });
        }
        let available = self.available(resource);
        if quantity > available {
            return Err(LedgerError::Exhausted {
                resource: resource.describe(),
                available: format!("{available}"),
                requested: format!("{quantity}"),
            });
        }
        self.balances.insert(resource.clone(), available - quantity);
        *self.spent.entry(resource.clone()).or_insert(0.0) += quantity;
        Ok(())
    }

    /// Moves the real-world allocation into a branch.
    ///
    /// Takes `self` by value. The parent is gone, so the allocation exists in exactly one place
    /// afterwards; a second material fork of the same ledger is a borrow-checker error rather
    /// than a validation finding. See the `MaterialCannotBeForkedTwice` doctest below.
    pub fn fork_material(self) -> TissueLedger {
        self
    }

    /// Opens a branch that may inspect and reason but not consume specimen.
    ///
    /// Borrows, so any number of these may coexist with each other and with the material
    /// branch. That is exactly 24.10's "forked simulations may inspect hypothetical outcomes".
    pub fn fork_hypothetical(&self) -> TissueLedger {
        TissueLedger {
            mode: BranchMode::Hypothetical,
            balances: self.balances.clone(),
            spent: self.spent.clone(),
        }
    }

    /// Refuses to merge two ledgers that each spent the same nonrenewable holding.
    ///
    /// The compile-time guarantee covers ledgers that were forked in this process. Ledgers
    /// arriving from a store or another site were not, so the same invariant needs a runtime
    /// form too.
    pub fn merge(left: &TissueLedger, right: &TissueLedger) -> Result<(), LedgerError> {
        let double_spent: BTreeSet<&ResourceId> = left
            .spent
            .iter()
            .filter(|(_, quantity)| **quantity > 0.0)
            .map(|(resource, _)| resource)
            .filter(|resource| {
                resource.kind.is_nonrenewable_material() && right.spent(resource) > 0.0
            })
            .collect();
        match double_spent.into_iter().next() {
            None => Ok(()),
            Some(resource) => Err(LedgerError::DuplicatedMaterial {
                resource: resource.describe(),
            }),
        }
    }
}

/// The λ weights of 24.10's utility expression.
///
/// Values here are placeholders. Section 24 states the shape of `U(a)` and states that resource
/// use is a first-class outcome dimension; it states no magnitudes, and this crate declines to
/// invent them. A benchmark that scores against utility must supply its own weights and publish
/// them, which is why the fields are public and there is no "recommended" constructor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UtilityWeights {
    pub cost: f64,
    pub time: f64,
    pub specimen: f64,
    pub privacy: f64,
}

impl Default for UtilityWeights {
    /// All weights 1.0: a stated placeholder, chosen because it is obviously arbitrary rather
    /// than plausibly tuned.
    fn default() -> Self {
        UtilityWeights {
            cost: 1.0,
            time: 1.0,
            specimen: 1.0,
            privacy: 1.0,
        }
    }
}

/// What one candidate action is expected to gain and to spend.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ActionEconomics {
    /// `E[ΔD(a)]`: expected improvement in the decision or in closed evidence obligations.
    pub expected_decision_gain: f64,
    /// `C(a)`: financial and compute cost.
    pub cost: f64,
    /// `T(a)`: time.
    pub time: f64,
    /// `S(a)`: specimen depletion.
    pub specimen: f64,
    /// `P(a)`: privacy or participant burden.
    pub privacy: f64,
}

impl ActionEconomics {
    /// Abstention: gains nothing, spends nothing, and scores exactly zero under any weights.
    /// It is the baseline every acquisition has to beat.
    pub fn abstain() -> Self {
        ActionEconomics::default()
    }
}

impl UtilityWeights {
    pub fn utility(&self, action: &ActionEconomics) -> f64 {
        action.expected_decision_gain
            - self.cost * action.cost
            - self.time * action.time
            - self.specimen * action.specimen
            - self.privacy * action.privacy
    }
}

/// The real-world allocation cannot be forked into two material branches.
///
/// ```compile_fail
/// use bioprism_foundation::ledger::TissueLedger;
/// let ledger = TissueLedger::material();
/// let _first = ledger.fork_material();
/// let _second = ledger.fork_material();
/// ```
///
/// Nor copied out from under the move.
///
/// ```compile_fail
/// use bioprism_foundation::ledger::TissueLedger;
/// let ledger = TissueLedger::material();
/// let _copy = ledger.clone();
/// ```
#[cfg(doctest)]
pub struct MaterialCannotBeForkedTwice;

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> ResourceId {
        ResourceId::new(ResourceKind::TissueMass, "ffpe-block:0001")
    }

    fn ledger() -> TissueLedger {
        TissueLedger::material().stock(block(), 3.0).unwrap()
    }

    #[test]
    fn consuming_more_sections_than_remain_reports_what_was_left() {
        let mut ledger = ledger();
        let err = ledger.consume(&block(), 4.0).unwrap_err();
        assert_eq!(
            err,
            LedgerError::Exhausted {
                resource: "ffpe-block:0001 (tissue mass)".to_string(),
                available: "3".to_string(),
                requested: "4".to_string()
            }
        );
    }

    #[test]
    fn a_hypothetical_branch_may_inspect_the_balance_but_never_consume_the_specimen() {
        let ledger = ledger();
        let mut hypothetical = ledger.fork_hypothetical();
        assert_eq!(hypothetical.available(&block()), 3.0);
        assert!(matches!(
            hypothetical.consume(&block(), 1.0).unwrap_err(),
            LedgerError::HypotheticalConsumption { .. }
        ));
    }

    #[test]
    fn a_hypothetical_branch_may_still_spend_renewable_budget() {
        let compute = ResourceId::new(ResourceKind::MonetaryCost, "gpu-hours");
        let ledger = TissueLedger::material().stock(compute.clone(), 100.0).unwrap();
        let mut hypothetical = ledger.fork_hypothetical();
        assert!(hypothetical.consume(&compute, 10.0).is_ok());
        assert_eq!(hypothetical.available(&compute), 90.0);
    }

    #[test]
    fn consumption_in_a_hypothetical_branch_never_reaches_the_material_ledger() {
        let ledger = ledger();
        let compute = ResourceId::new(ResourceKind::TurnaroundTime, "days");
        let ledger = ledger.stock(compute.clone(), 10.0).unwrap();
        let mut hypothetical = ledger.fork_hypothetical();
        hypothetical.consume(&compute, 5.0).unwrap();
        assert_eq!(ledger.available(&compute), 10.0);
    }

    #[test]
    fn two_ledgers_that_each_spent_the_same_block_cannot_be_merged() {
        let mut left = ledger();
        let mut right = ledger();
        left.consume(&block(), 1.0).unwrap();
        right.consume(&block(), 1.0).unwrap();
        assert!(matches!(
            TissueLedger::merge(&left, &right).unwrap_err(),
            LedgerError::DuplicatedMaterial { .. }
        ));
    }

    #[test]
    fn two_ledgers_spending_only_renewable_budget_merge_without_complaint() {
        let hours = ResourceId::new(ResourceKind::LaboratoryOrExpertTime, "reviewer-hours");
        let mut left = TissueLedger::material().stock(hours.clone(), 8.0).unwrap();
        let mut right = TissueLedger::material().stock(hours.clone(), 8.0).unwrap();
        left.consume(&hours, 2.0).unwrap();
        right.consume(&hours, 3.0).unwrap();
        assert!(TissueLedger::merge(&left, &right).is_ok());
    }

    #[test]
    fn a_negative_or_infinite_quantity_is_refused_rather_than_silently_clamped() {
        let mut ledger = ledger();
        assert!(ledger.consume(&block(), -1.0).is_err());
        assert!(ledger.consume(&block(), f64::INFINITY).is_err());
        assert!(TissueLedger::material().stock(block(), f64::NAN).is_err());
    }

    #[test]
    fn an_assay_that_consumes_tissue_and_gains_nothing_scores_below_abstention() {
        let weights = UtilityWeights::default();
        let wasteful = ActionEconomics {
            expected_decision_gain: 0.0,
            specimen: 1.0,
            ..ActionEconomics::default()
        };
        assert!(weights.utility(&wasteful) < weights.utility(&ActionEconomics::abstain()));
    }

    #[test]
    fn between_two_equally_informative_assays_the_one_sparing_tissue_wins() {
        let weights = UtilityWeights::default();
        let destructive = ActionEconomics {
            expected_decision_gain: 0.5,
            specimen: 1.0,
            ..ActionEconomics::default()
        };
        let sparing = ActionEconomics {
            expected_decision_gain: 0.5,
            specimen: 0.0,
            ..ActionEconomics::default()
        };
        assert!(weights.utility(&sparing) > weights.utility(&destructive));
    }

    #[test]
    fn no_positive_weighting_makes_indiscriminate_measurement_free() {
        for specimen_weight in [0.1_f64, 1.0, 10.0] {
            let weights = UtilityWeights {
                specimen: specimen_weight,
                ..UtilityWeights::default()
            };
            let action = ActionEconomics {
                expected_decision_gain: 0.0,
                specimen: 2.0,
                ..ActionEconomics::default()
            };
            assert!(weights.utility(&action) < 0.0);
        }
    }

    #[test]
    fn the_material_branch_retains_the_allocation_it_was_moved_into() {
        let mut moved = ledger().fork_material();
        assert_eq!(moved.available(&block()), 3.0);
        moved.consume(&block(), 3.0).unwrap();
        assert_eq!(moved.available(&block()), 0.0);
        assert_eq!(moved.spent(&block()), 3.0);
    }
}
