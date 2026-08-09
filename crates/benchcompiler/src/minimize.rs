//! State and context minimization with an explicit preserved property.
//!
//! Blueprint 06.07. Delta debugging is easy; delta debugging that is *correct* is the whole
//! module. A reduction is only meaningful if the reason the cell was interesting survives every
//! removal, so the reason is not inferred here — it is supplied by the caller as an
//! [`InterestProbe`], and the only thing this module guarantees is that the probe's answer on the
//! reduced context equals its answer on the original.
//!
//! Three consequences follow, and they are the reason this is not simply `prism::minimize` with a
//! different argument type:
//!
//! - **Losing the property is an error, not a smaller cell.** A reduction that changed what the
//!   context demonstrates is not a weaker result; it is a wrong one, and it returns
//!   [`MinimizeError::PropertyLost`] rather than a `Minimization` with a caveat field.
//! - **1-minimality is proven, not asserted.** The greedy loop runs to a fixpoint and is then
//!   followed by an independent verification pass that re-probes every remaining removable unit.
//!   Each probe that fails to reproduce the signature is recorded as a [`MinimalityWitness`], so
//!   the claim ships with its evidence. `prism::minimize` runs one pass and states the weaker
//!   guarantee honestly; this one pays for the stronger claim.
//! - **Some context is load-bearing for task intent and is never removed.** 06.07's semantic guard:
//!   context that does not change the observed behaviour may still be what makes the task the task.
//!   Pinned items are excluded from the minimality claim rather than quietly counted in it.
//!
//! Removal is hierarchical (06.07's ordering: services and directories before artifacts, regions,
//! turns, memory entries, tool methods and fields) and deterministic: items are visited in
//! `(tier, id)` order, so the same input always yields the same minimal set. Nothing here consults
//! a clock or a random source.
//!
//! **Deliberately not implemented.** 06.07's "probabilistic preservation" — comparing behaviour
//! *distributions* for stochastic models under a declared equivalence threshold — is absent. It
//! needs repeated trials and a sampling budget this crate has no way to spend, and approximating it
//! with a single probe would silently turn a statistical claim into a point estimate. A caller with
//! a stochastic system should aggregate trials inside its own probe and return a signature that
//! already encodes the equivalence decision.

use crate::error::MinimizeError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The granularity of a piece of context, coarse to fine.
///
/// Ordering is the removal order of 06.07: throwing away a whole unrelated service is one probe
/// that can retire hundreds of fields, so it is attempted first. The variants are declared in that
/// order and `Ord` is derived from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Service,
    Directory,
    Artifact,
    DocumentRegion,
    HistoryTurn,
    MemoryEntry,
    ToolMethod,
    Field,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Service => "service",
            Tier::Directory => "directory",
            Tier::Artifact => "artifact",
            Tier::DocumentRegion => "document_region",
            Tier::HistoryTurn => "history_turn",
            Tier::MemoryEntry => "memory_entry",
            Tier::ToolMethod => "tool_method",
            Tier::Field => "field",
        }
    }
}

/// Whether an item may be removed at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Guard {
    Removable,
    /// 06.07's semantic guard. The item defines what the task *is*; removing it would leave a
    /// context that still reproduces the behaviour while no longer posing the question.
    TaskIntent,
}

/// One removable piece of the parent execution's context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextItem {
    pub id: String,
    pub tier: Tier,
    pub guard: Guard,
    /// The item that contains this one. Removing a container removes everything inside it, which
    /// is what makes coarse-first ordering worth anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

impl ContextItem {
    pub fn new(id: impl Into<String>, tier: Tier) -> Self {
        ContextItem {
            id: id.into(),
            tier,
            guard: Guard::Removable,
            parent: None,
        }
    }

    /// Marks the item as defining task intent, so minimization will never remove it.
    pub fn pinned(mut self) -> Self {
        self.guard = Guard::TaskIntent;
        self
    }

    pub fn inside(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }
}

/// What makes a context interesting, as an observable value.
///
/// Deliberately structured rather than a bare boolean. "The failure still reproduces" and "the
/// failure still reproduces *for the same reason*" are different properties, and a reduction that
/// preserves the first while destroying the second is the classic way delta debugging produces a
/// small artifact that no longer diagnoses anything.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InterestSignature {
    /// The verdict the context produces, in the caller's vocabulary.
    pub verdict: String,
    /// Concrete checkable objects the verdict rests on. Order-insensitive by construction.
    pub witnesses: BTreeSet<String>,
    /// The step at which the behaviour of interest appears, where the caller tracks one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divergence_step: Option<usize>,
}

impl InterestSignature {
    pub fn new(verdict: impl Into<String>) -> Self {
        InterestSignature {
            verdict: verdict.into(),
            witnesses: BTreeSet::new(),
            divergence_step: None,
        }
    }

    pub fn with_witness(mut self, witness: impl Into<String>) -> Self {
        self.witnesses.insert(witness.into());
        self
    }

    pub fn at_step(mut self, step: usize) -> Self {
        self.divergence_step = Some(step);
        self
    }

    /// A one-line rendering for error messages, so a diagnostic names both signatures.
    pub fn describe(&self) -> String {
        let witnesses = if self.witnesses.is_empty() {
            "no witnesses".to_string()
        } else {
            self.witnesses
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join("+")
        };
        match self.divergence_step {
            Some(step) => format!("{}[{}]@{}", self.verdict, witnesses, step),
            None => format!("{}[{}]", self.verdict, witnesses),
        }
    }
}

/// The caller's answer to "is this context still interesting, and in the same way?".
///
/// Taking a probe rather than a fixed predicate is the point of the module: minimization has no
/// opinion about what makes a cell worth keeping, and a crate that guessed would be preserving its
/// own guess rather than the caller's finding.
pub trait InterestProbe {
    fn observe(&mut self, kept: &BTreeSet<String>) -> InterestSignature;
}

impl<F> InterestProbe for F
where
    F: FnMut(&BTreeSet<String>) -> InterestSignature,
{
    fn observe(&mut self, kept: &BTreeSet<String>) -> InterestSignature {
        self(kept)
    }
}

/// Evidence that one remaining unit is load-bearing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimalityWitness {
    /// The item whose removal was attempted during verification.
    pub unit_root: String,
    /// The item and everything it contains, all of which would have gone with it.
    pub would_remove: Vec<String>,
    /// What the probe reported without them. Differs from the preserved signature, which is why
    /// the unit stayed.
    pub observed_without: InterestSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Minimization {
    pub started_from: usize,
    /// The reduced context, sorted for determinism.
    pub minimal: Vec<String>,
    pub removed: Vec<String>,
    /// Items retained by the semantic guard rather than by the probe. Excluded from the
    /// minimality claim, and named so nobody mistakes them for load-bearing evidence.
    pub pinned: Vec<String>,
    pub preserved: InterestSignature,
    /// One per remaining removable unit. An empty vector on a non-empty result means every
    /// remaining item is pinned.
    pub minimality_witnesses: Vec<MinimalityWitness>,
    /// Probes performed, including the determinism check and the verification pass. Reported
    /// because the cost of minimizing is part of whether it was worth doing.
    pub evaluations: usize,
    /// Greedy passes run before the fixpoint. More than one means a later removal unlocked an
    /// earlier one, which a single-pass minimizer would have missed.
    pub passes: usize,
    pub guarantee: String,
}

impl Minimization {
    /// Fraction of the original context that survived. Lower is a sharper diagnosis.
    pub fn reduction_ratio(&self) -> f64 {
        if self.started_from == 0 {
            return 1.0;
        }
        self.minimal.len() as f64 / self.started_from as f64
    }

    /// Re-runs the probe against the reduced context.
    ///
    /// The caller's own check, independent of the one minimization already performed. 35.08's
    /// quality gates want a second party to reproduce a release candidate; this is the same idea at
    /// the scale of a single reduction.
    pub fn preserves_under<P: InterestProbe>(&self, probe: &mut P) -> bool {
        let kept: BTreeSet<String> = self.minimal.iter().cloned().collect();
        probe.observe(&kept) == self.preserved
    }
}

/// How much probing a minimization may spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinimizeBudget {
    pub max_evaluations: usize,
}

impl Default for MinimizeBudget {
    fn default() -> Self {
        MinimizeBudget {
            max_evaluations: 4096,
        }
    }
}

struct Forest {
    /// Every item, by id, in `(tier, id)` visit order.
    order: Vec<String>,
    unit: BTreeMap<String, Vec<String>>,
    pinned: BTreeSet<String>,
}

fn build_forest(items: &[ContextItem]) -> Result<Forest, MinimizeError> {
    let by_id: BTreeMap<&str, &ContextItem> =
        items.iter().map(|item| (item.id.as_str(), item)).collect();

    for item in items {
        if let Some(parent) = &item.parent {
            if !by_id.contains_key(parent.as_str()) {
                return Err(MinimizeError::DanglingParent {
                    id: item.id.clone(),
                    parent: parent.clone(),
                });
            }
        }
    }

    for item in items {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        seen.insert(item.id.as_str());
        let mut cursor = item.parent.as_deref();
        while let Some(parent) = cursor {
            if !seen.insert(parent) {
                return Err(MinimizeError::CyclicContainment {
                    id: item.id.clone(),
                });
            }
            cursor = by_id
                .get(parent)
                .and_then(|item| item.parent.as_deref());
        }
    }

    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for item in items {
        if let Some(parent) = &item.parent {
            children
                .entry(parent.as_str())
                .or_default()
                .push(item.id.as_str());
        }
    }

    let mut unit: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for item in items {
        let mut collected: BTreeSet<String> = BTreeSet::new();
        let mut stack = vec![item.id.as_str()];
        while let Some(current) = stack.pop() {
            if !collected.insert(current.to_string()) {
                continue;
            }
            for child in children.get(current).into_iter().flatten() {
                stack.push(child);
            }
        }
        unit.insert(item.id.clone(), collected.into_iter().collect());
    }

    let mut ordered: Vec<&ContextItem> = items.iter().collect();
    ordered.sort_by(|a, b| a.tier.cmp(&b.tier).then_with(|| a.id.cmp(&b.id)));

    let pinned: BTreeSet<String> = items
        .iter()
        .filter(|item| item.guard == Guard::TaskIntent)
        .map(|item| item.id.clone())
        .collect();

    Ok(Forest {
        order: ordered.into_iter().map(|item| item.id.clone()).collect(),
        unit,
        pinned,
    })
}

impl Forest {
    /// Whether a unit may be removed as a whole.
    ///
    /// A container holding a pinned item is not removable, however unremarkable the container
    /// itself is: removing it would take the pinned item with it and defeat the guard.
    fn removable(&self, root: &str) -> bool {
        self.unit
            .get(root)
            .map(|members| members.iter().all(|id| !self.pinned.contains(id)))
            .unwrap_or(false)
    }
}

struct Meter<'a, P: InterestProbe> {
    probe: &'a mut P,
    spent: usize,
    budget: usize,
}

impl<P: InterestProbe> Meter<'_, P> {
    fn observe(&mut self, kept: &BTreeSet<String>) -> Result<InterestSignature, MinimizeError> {
        if self.spent >= self.budget {
            return Err(MinimizeError::BudgetExhausted {
                budget: self.budget,
                spent: self.spent,
            });
        }
        self.spent += 1;
        Ok(self.probe.observe(kept))
    }
}

/// Reduces `items` to a 1-minimal context preserving whatever `probe` reports for the whole set.
pub fn minimize<P: InterestProbe>(
    items: &[ContextItem],
    probe: &mut P,
    budget: MinimizeBudget,
) -> Result<Minimization, MinimizeError> {
    minimize_inner(items, probe, None, budget)
}

/// Reduces `items` while preserving a property the caller states up front.
///
/// The difference from [`minimize`] is not the algorithm but the failure mode: if the starting
/// context does not exhibit `expected`, this refuses to run. Minimizing a context that never showed
/// the behaviour yields a small context that still does not show it, which reads as a successful
/// reduction and is worse than an error.
pub fn minimize_preserving<P: InterestProbe>(
    items: &[ContextItem],
    probe: &mut P,
    expected: &InterestSignature,
    budget: MinimizeBudget,
) -> Result<Minimization, MinimizeError> {
    minimize_inner(items, probe, Some(expected), budget)
}

fn minimize_inner<P: InterestProbe>(
    items: &[ContextItem],
    probe: &mut P,
    expected: Option<&InterestSignature>,
    budget: MinimizeBudget,
) -> Result<Minimization, MinimizeError> {
    if items.is_empty() {
        return Err(MinimizeError::NothingToMinimize);
    }
    let forest = build_forest(items)?;
    let mut meter = Meter {
        probe,
        spent: 0,
        budget: budget.max_evaluations,
    };

    let mut kept: BTreeSet<String> = forest.order.iter().cloned().collect();
    let target = meter.observe(&kept)?;
    let confirm = meter.observe(&kept)?;
    if confirm != target {
        return Err(MinimizeError::NondeterministicProbe {
            size: kept.len(),
            first: target.describe(),
            second: confirm.describe(),
        });
    }
    if let Some(expected) = expected {
        if &target != expected {
            return Err(MinimizeError::NotInterestingToBeginWith {
                expected: expected.describe(),
                observed: target.describe(),
            });
        }
    }

    let mut removed: BTreeSet<String> = BTreeSet::new();
    let mut passes = 0usize;
    loop {
        passes += 1;
        let mut removed_this_pass = false;
        for root in &forest.order {
            if !kept.contains(root) || !forest.removable(root) {
                continue;
            }
            let unit = &forest.unit[root];
            let mut attempt = kept.clone();
            for member in unit {
                attempt.remove(member);
            }
            if attempt.len() == kept.len() {
                continue;
            }
            if meter.observe(&attempt)? == target {
                for member in unit {
                    if kept.remove(member) {
                        removed.insert(member.clone());
                    }
                }
                removed_this_pass = true;
            }
        }
        if !removed_this_pass {
            break;
        }
    }

    let final_check = meter.observe(&kept)?;
    if final_check != target {
        return Err(MinimizeError::property_lost(&target, &final_check));
    }

    let mut witnesses = Vec::new();
    for root in &forest.order {
        if !kept.contains(root) || !forest.removable(root) {
            continue;
        }
        let would_remove: Vec<String> = forest.unit[root]
            .iter()
            .filter(|member| kept.contains(*member))
            .cloned()
            .collect();
        let mut attempt = kept.clone();
        for member in &would_remove {
            attempt.remove(member);
        }
        let observed = meter.observe(&attempt)?;
        if observed == target {
            return Err(MinimizeError::NotOneMinimal {
                unit: root.clone(),
            });
        }
        witnesses.push(MinimalityWitness {
            unit_root: root.clone(),
            would_remove,
            observed_without: observed,
        });
    }

    let pinned: Vec<String> = forest
        .pinned
        .iter()
        .filter(|id| kept.contains(*id))
        .cloned()
        .collect();

    Ok(Minimization {
        started_from: items.len(),
        minimal: kept.iter().cloned().collect(),
        removed: removed.into_iter().collect(),
        pinned,
        preserved: target,
        minimality_witnesses: witnesses,
        evaluations: meter.spent,
        passes,
        guarantee: "1-minimal over removable units, proven by a verification pass: for each \
                    remaining unremovable-by-guard unit there is a recorded probe showing the \
                    preserved signature does not survive its removal. Items pinned to task intent \
                    are exempt and are listed separately. Not globally minimal; that search is \
                    exponential in the number of items."
            .to_string(),
    })
}
