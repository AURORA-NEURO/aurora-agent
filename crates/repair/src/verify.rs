//! Checking a claimed repair against a plan's own declared criteria.
//!
//! Verification does one thing and refuses to do a second: it reports **which declared criteria
//! held**. It never reports that the issue is resolved. The gap between "every declared criterion
//! held" and "the issue is fixed" belongs to whoever wrote the criteria, and every report says so
//! in its own limitations rather than leaving a reader to infer it.
//!
//! # Staleness comes first, and nothing is evaluated behind it
//!
//! A plan is bound to the world it was made from. If the world offered for verification is not
//! that world, [`verify`] returns [`AcceptanceReport::Stale`] and evaluates nothing at all — not
//! "evaluates and flags", because a verdict computed against a different world is not a verdict
//! about this plan, and a report carrying both a verdict and a staleness flag invites a reader to
//! take the verdict and skip the flag.
//!
//! This is the same conclusion `bioprism_tokens::staleness` reached for compiled context, for the
//! same reason its module documentation gives: *stale context is never silently reused*, with
//! "silently" the load-bearing word. That module judges currency against a caller-supplied epoch
//! or an observed world digest and never against a clock, and it needs a third
//! `Undetermined` state for the caller who supplied no reference. This crate needs no such state:
//! the verifier holds both digests by construction — the plan carries one and the world computes
//! the other — so "I could not check" cannot arise here. No dependency is taken on that crate; the
//! question here is a two-sided digest comparison, not a TTL model, and importing five currency
//! variants to express two would be borrowing a vocabulary rather than reusing a mechanism.
//!
//! # Verifying a repaired tree
//!
//! A repaired tree is, necessarily, a different world: the project world id is derived from the
//! file listing, so any edit changes it. [`verify`] therefore reports `Stale` for exactly the
//! situation the tool exists for — which is correct, and is why [`verify_successor`] exists. It
//! takes a [`Succession`]: a human's signed statement that this new world is the repaired
//! successor of the planned one. The tool cannot know that; only a person can assert it. The
//! report carries the assertion verbatim and a limitation saying it was asserted and not verified.
//! There is deliberately no way to get a verdict against a different world without someone's name
//! attached to the claim that it is the right one.
//!
//! # The value map
//!
//! Predicates are evaluated against `variable -> value` over **every fact in the world**, built the
//! way `bioprism-fiber` builds the map it hands an oracle. It is not restricted to the plan's bound
//! region: the region binding records what evidence the plan was made from, and using it to blind
//! the checker would make "the compiler judged this irrelevant" and "the variable is gone" arrive
//! as the same `NotEvaluable`. Those are different states, so they get different treatment. The
//! cost — verification is not budget-aware and does not recompile a region — is on every report.

use crate::plan::{
    required_str, strict_object, string_array, AcceptanceCriterion, Falsifier, Obligation, Origin,
    RepairPlan,
};
use crate::RepairError;
use bioprism_domain::rules::Obstruction;
use bioprism_domain::Predicate;
use bioprism_world::World;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The report document's schema version.
pub const REPORT_SCHEMA_VERSION: &str = "bioprism-repair-report/0.1";

/// `variable -> value` over every fact in a world.
///
/// The same mapping `bioprism-fiber` hands a `DecisionOracle`, over the whole world rather than a
/// compiled region.
///
/// **This map can lose a fact, and the loss is reported rather than assumed away.**
/// `bioprism_world::World::from_json` does *not* refuse a world in which two facts provide the
/// same variable — `validate_reference_compat` checks duplicate ids and factor inputs, and
/// shadowing is an error the separate reference validator `bioprism_world::validate` raises, with
/// its own message saying "the last one silently wins, so the compiled decision section depends on
/// document order". Collecting into a map reproduces exactly that: the last fact in document order
/// wins and the earlier value is gone. Since this crate cannot claim a criterion was checked
/// against the world when it was checked against one of two candidate values, [`verify`] names
/// every shadowed variable in the report's limitations instead of leaving the collapse silent.
pub fn world_value_map(world: &World) -> BTreeMap<String, Value> {
    world
        .facts
        .iter()
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect()
}

/// A caller's assertion that one world is the repaired successor of another.
///
/// Private fields with one constructor, so a succession cannot exist without a name attached to
/// it. [`Succession::declare`] refuses an empty declarant or an empty statement: "someone said so"
/// with nobody saying it is the shape of an unowned claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Succession {
    declared_by: String,
    statement: String,
}

impl Succession {
    pub fn declare(
        declared_by: impl Into<String>,
        statement: impl Into<String>,
    ) -> Result<Succession, RepairError> {
        let declared_by = declared_by.into();
        let statement = statement.into();
        if declared_by.trim().is_empty() {
            return Err(RepairError::EmptyField {
                what: "succession.declared_by".into(),
            });
        }
        if statement.trim().is_empty() {
            return Err(RepairError::EmptyField {
                what: "succession.statement".into(),
            });
        }
        Ok(Succession {
            declared_by,
            statement,
        })
    }

    pub fn declared_by(&self) -> &str {
        &self.declared_by
    }

    pub fn statement(&self) -> &str {
        &self.statement
    }
}

/// Which list an item came from. Carried on every reported status so one flat list of statuses
/// stays unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    Criterion,
    Obligation,
    Falsifier,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ItemKind::Criterion => "criterion",
            ItemKind::Obligation => "obligation",
            ItemKind::Falsifier => "falsifier",
        }
    }

    fn parse(text: &str) -> Result<ItemKind, RepairError> {
        match text {
            "criterion" => Ok(ItemKind::Criterion),
            "obligation" => Ok(ItemKind::Obligation),
            "falsifier" => Ok(ItemKind::Falsifier),
            other => Err(RepairError::Document(format!(
                "item kind must be \"criterion\", \"obligation\" or \"falsifier\", found {other:?}"
            ))),
        }
    }
}

/// What one item's predicate said about the verified world.
///
/// Three values, and [`ItemStatus::NotEvaluable`] is never folded into either of the others. It
/// carries the [`Obstruction`] `bioprism-domain` produced, so the report names the variable that
/// stopped the check rather than reporting that something, somewhere, went unchecked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemStatus {
    Met,
    Unmet,
    NotEvaluable(Obstruction),
}

impl ItemStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemStatus::Met => "met",
            ItemStatus::Unmet => "unmet",
            ItemStatus::NotEvaluable(_) => "not_evaluable",
        }
    }

    /// The obstruction, when there is one. There is no `is_met_or_default`: a caller that wants to
    /// treat an unevaluable item as anything must write that decision down itself.
    pub fn obstruction(&self) -> Option<&Obstruction> {
        match self {
            ItemStatus::NotEvaluable(obstruction) => Some(obstruction),
            _ => None,
        }
    }

    fn of(predicate: &Predicate, values: &BTreeMap<String, Value>) -> ItemStatus {
        match predicate.evaluate(values) {
            Ok(true) => ItemStatus::Met,
            Ok(false) => ItemStatus::Unmet,
            Err(obstruction) => ItemStatus::NotEvaluable(obstruction),
        }
    }
}

/// One item's individual result, with everything a reader needs to check it by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemOutcome {
    pub kind: ItemKind,
    pub name: String,
    /// The plan's prose, verbatim, so a status quoted alone still says what was checked.
    pub statement: String,
    pub origin: Origin,
    pub status: ItemStatus,
}

/// The achievement verdict.
///
/// # The ordering, and why it is this one
///
/// 1. **`Falsified`** — some falsifier held. This outranks everything, including
///    `Underdetermined`, because of an asymmetry in what the verdicts presuppose. `Falsified` is
///    not a claim about the criterion set at all; it is a single determinate observation that the
///    plan was the wrong plan. An unevaluable criterion cannot undermine it, because the verdict
///    never claimed the criteria were adjudicated. A plan proven wrong does not become
///    less wrong because one of its criteria could not be checked.
/// 2. **`Underdetermined`** — some criterion or falsifier could not be evaluated. This outranks
///    `NotMet`, and that is the ordering worth arguing about, since the workspace's rule elsewhere
///    is that a proven violation outranks a blind check. The difference is what the verdict
///    presupposes. `NotMet` says *the criteria were checked and not all of them held*, and a reader
///    who is told that may reasonably conclude that clearing the failures is the whole remaining
///    distance to `Met`. When a criterion never ran, that conclusion is false: clearing the
///    failures could still leave an unknown. `Underdetermined` refuses the inference. Nothing is
///    lost by it — every determinate failure is still on its own item with status `Unmet`, which is
///    where a reader acts from. A falsifier that could not be evaluated lands here too: nobody
///    checked whether the plan is wrong, and that is not the same as the plan being right.
/// 3. **`NotMet`** — everything was evaluated and some criterion did not hold.
/// 4. **`Met`** — every declared criterion held. It means exactly that and nothing more; the
///    report's limitations say so in full.
///
/// Obligations are not in this ordering. See [`AcceptanceReport::admissibility`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Outcome {
    Falsified,
    Underdetermined,
    NotMet,
    Met,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Falsified => "falsified",
            Outcome::Underdetermined => "underdetermined",
            Outcome::NotMet => "not_met",
            Outcome::Met => "met",
        }
    }

    fn parse(text: &str) -> Result<Outcome, RepairError> {
        match text {
            "falsified" => Ok(Outcome::Falsified),
            "underdetermined" => Ok(Outcome::Underdetermined),
            "not_met" => Ok(Outcome::NotMet),
            "met" => Ok(Outcome::Met),
            other => Err(RepairError::Document(format!(
                "unknown outcome {other:?}"
            ))),
        }
    }
}

/// Whether the plan's declared prerequisites held — reported on its own axis, never folded into
/// [`Outcome`].
///
/// An obligation asks whether the change was admissible **to make**, which is a question about the
/// moment before the change. This verifier only ever sees one world, so it checks obligations
/// retrospectively, which is a weaker check than the plan asserted. Letting a weaker check move
/// the achievement verdict would contaminate it; hiding the obligations entirely would be worse.
/// So they get their own value, with the same ordering discipline as [`Outcome`].
///
/// [`Admissibility::Undeclared`] is a real state and not a synonym for `Held`: a plan that declares
/// no prerequisites has declared none, which is different from having declared that none are
/// needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Admissibility {
    Undeclared,
    Undetermined,
    Violated,
    Held,
}

impl Admissibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Admissibility::Undeclared => "undeclared",
            Admissibility::Undetermined => "undetermined",
            Admissibility::Violated => "violated",
            Admissibility::Held => "held",
        }
    }

    fn parse(text: &str) -> Result<Admissibility, RepairError> {
        match text {
            "undeclared" => Ok(Admissibility::Undeclared),
            "undetermined" => Ok(Admissibility::Undetermined),
            "violated" => Ok(Admissibility::Violated),
            "held" => Ok(Admissibility::Held),
            other => Err(RepairError::Document(format!(
                "unknown admissibility {other:?}"
            ))),
        }
    }
}

/// What one verification concluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcceptanceReport {
    /// The world offered is not the world the plan was bound to. Nothing was evaluated.
    Stale {
        plan_id: String,
        issue_id: String,
        expected_world_id: String,
        found_world_id: String,
        expected_world_sha256: String,
        found_world_sha256: String,
        limitations: Vec<String>,
    },
    Evaluated {
        plan_id: String,
        issue_id: String,
        goal: String,
        world_id: String,
        world_sha256: String,
        /// Whether the verified world is byte-for-byte the world the plan was bound to. False only
        /// under [`verify_successor`], where a caller declared the succession.
        binding_matches: bool,
        succession: Option<Succession>,
        outcome: Outcome,
        admissibility: Admissibility,
        /// Fact ids the plan's region binding names that no longer exist in the verified world.
        missing_region_facts: Vec<String>,
        /// Every item, criteria first, then obligations, then falsifiers, each in the plan's own
        /// order. Nothing is reordered here: [`crate::plan_for_issue`] sorts each of its three
        /// lists by name, so a generated plan's report reads name-ordered, but a hand-authored or
        /// parsed plan is reported in the order its author wrote — the report does not rearrange a
        /// document it did not write.
        items: Vec<ItemOutcome>,
        limitations: Vec<String>,
    },
}

impl AcceptanceReport {
    pub fn plan_id(&self) -> &str {
        match self {
            AcceptanceReport::Stale { plan_id, .. } => plan_id,
            AcceptanceReport::Evaluated { plan_id, .. } => plan_id,
        }
    }

    pub fn issue_id(&self) -> &str {
        match self {
            AcceptanceReport::Stale { issue_id, .. } => issue_id,
            AcceptanceReport::Evaluated { issue_id, .. } => issue_id,
        }
    }

    /// The achievement verdict, when there is one. `None` for a stale report, because a stale
    /// report has no verdict rather than a neutral one.
    pub fn outcome(&self) -> Option<Outcome> {
        match self {
            AcceptanceReport::Stale { .. } => None,
            AcceptanceReport::Evaluated { outcome, .. } => Some(*outcome),
        }
    }

    pub fn admissibility(&self) -> Option<Admissibility> {
        match self {
            AcceptanceReport::Stale { .. } => None,
            AcceptanceReport::Evaluated { admissibility, .. } => Some(*admissibility),
        }
    }

    pub fn items(&self) -> &[ItemOutcome] {
        match self {
            AcceptanceReport::Stale { .. } => &[],
            AcceptanceReport::Evaluated { items, .. } => items,
        }
    }

    /// One item by name, whatever list it came from. Names are unique across the whole plan.
    pub fn item(&self, name: &str) -> Option<&ItemOutcome> {
        self.items().iter().find(|item| item.name == name)
    }

    pub fn items_of(&self, kind: ItemKind) -> impl Iterator<Item = &ItemOutcome> {
        self.items().iter().filter(move |item| item.kind == kind)
    }

    pub fn limitations(&self) -> &[String] {
        match self {
            AcceptanceReport::Stale { limitations, .. } => limitations,
            AcceptanceReport::Evaluated { limitations, .. } => limitations,
        }
    }

    /// The bound region's fact ids that no longer exist in the verified world.
    ///
    /// Empty for a stale report, where nothing about the offered world was inspected at all.
    pub fn missing_region_facts(&self) -> &[String] {
        match self {
            AcceptanceReport::Stale { .. } => &[],
            AcceptanceReport::Evaluated {
                missing_region_facts,
                ..
            } => missing_region_facts,
        }
    }

    /// One line for a log. Names every item's status, because an aggregate that hides which item
    /// could not run is the failure mode this crate exists to avoid.
    pub fn summary(&self) -> String {
        match self {
            AcceptanceReport::Stale {
                plan_id,
                expected_world_id,
                found_world_id,
                ..
            } => format!(
                "{plan_id} STALE: bound to world {expected_world_id}, offered {found_world_id}; \
                 nothing evaluated"
            ),
            AcceptanceReport::Evaluated {
                plan_id,
                outcome,
                admissibility,
                items,
                ..
            } => format!(
                "{plan_id} {} (admissibility {}) [{}]",
                outcome.as_str(),
                admissibility.as_str(),
                items
                    .iter()
                    .map(|item| match &item.status {
                        ItemStatus::NotEvaluable(obstruction) => format!(
                            "{} {}={} ({} {})",
                            item.kind.as_str(),
                            item.name,
                            item.status.as_str(),
                            obstruction.variable,
                            obstruction.reason
                        ),
                        _ => format!(
                            "{} {}={}",
                            item.kind.as_str(),
                            item.name,
                            item.status.as_str()
                        ),
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        }
    }
}

/// Checks a claimed repair against the plan, in the world the plan was bound to.
///
/// Returns [`AcceptanceReport::Stale`] without evaluating anything if the world differs. To check
/// a repaired tree — which is by construction a different world — use [`verify_successor`].
pub fn verify(plan: &RepairPlan, world: &World) -> AcceptanceReport {
    verify_inner(plan, world, None)
}

/// Checks a claimed repair against a world a caller declares to be the repaired successor of the
/// world the plan was bound to.
///
/// The succession is recorded verbatim on the report and is never verified: nothing here can know
/// that two scanned trees are the same project before and after a change.
pub fn verify_successor(
    plan: &RepairPlan,
    world: &World,
    succession: &Succession,
) -> AcceptanceReport {
    verify_inner(plan, world, Some(succession))
}

fn verify_inner(
    plan: &RepairPlan,
    world: &World,
    succession: Option<&Succession>,
) -> AcceptanceReport {
    let binding = plan.evidence_binding();
    let found_world_id = world.world_id.as_str().to_string();
    let found_world_sha256 = world.content_hash().as_str().to_string();
    let binding_matches =
        binding.world_id == found_world_id && binding.world_sha256 == found_world_sha256;

    if !binding_matches && succession.is_none() {
        let mut limitations = plan.limitations().to_vec();
        limitations.push(STALE_LIMITATION.to_string());
        return AcceptanceReport::Stale {
            plan_id: plan.plan_id().to_string(),
            issue_id: plan.issue_id().to_string(),
            expected_world_id: binding.world_id.clone(),
            found_world_id,
            expected_world_sha256: binding.world_sha256.clone(),
            found_world_sha256,
            limitations,
        };
    }

    let values = world_value_map(world);
    let mut items: Vec<ItemOutcome> = Vec::new();
    for criterion in plan.criteria() {
        items.push(criterion_outcome(criterion, &values));
    }
    for obligation in plan.obligations() {
        items.push(obligation_outcome(obligation, &values));
    }
    for falsifier in plan.falsifiers() {
        items.push(falsifier_outcome(falsifier, &values));
    }

    let outcome = decide(&items);
    let admissibility = decide_admissibility(&items);
    let missing_region_facts: Vec<String> = binding
        .region_fact_ids
        .iter()
        .filter(|id| world.fact(id).is_none())
        .cloned()
        .collect();

    let mut limitations = plan.limitations().to_vec();
    limitations.extend(REPORT_LIMITATIONS.iter().map(|line| line.to_string()));
    if let Some(succession) = succession {
        // Branched on `binding_matches` because [`verify_successor`] accepts the planned world
        // itself, and a report that said "the world verified here is not the world this plan was
        // bound to" beside `binding_matches: true` would be stating the opposite of its own field.
        // The declaration is still recorded: it was made, and a report that dropped it would hide
        // that somebody was prepared to vouch for a succession.
        limitations.push(if binding_matches {
            format!(
                "A succession was declared, and it was not relied on: the world verified here is \
                 byte-for-byte the world this plan was bound to. {} declared it the successor: \
                 {}. The declaration is recorded because it was made, not because a verdict \
                 rested on it.",
                succession.declared_by(),
                succession.statement()
            )
        } else {
            format!(
                "The world verified here is not the world this plan was bound to. {} declared it \
                 the successor: {}. That succession is asserted by the caller and is never \
                 verified: nothing in this crate can know that two scanned trees are the same \
                 project before and after a change.",
                succession.declared_by(),
                succession.statement()
            )
        });
    }
    if !missing_region_facts.is_empty() {
        limitations.push(format!(
            "The plan's evidence region names {} fact(s) that no longer exist in the verified \
             world: {}.",
            missing_region_facts.len(),
            missing_region_facts.join(", ")
        ));
    }
    // Sorted and deduplicated because `WorldIndex::shadowed_variables` is built in document order
    // and repeats a variable once per extra fact providing it, and neither of those may reach a
    // report's bytes.
    let mut shadowed = world.index().shadowed_variables.clone();
    shadowed.sort();
    shadowed.dedup();
    if !shadowed.is_empty() {
        limitations.push(format!(
            "The verified world provides {} variable(s) from more than one fact ({}). The value \
             map keeps the last fact in document order for each, so any item reading one of them \
             was evaluated against a value document order picked, not against the world.",
            shadowed.len(),
            shadowed.join(", ")
        ));
    }

    AcceptanceReport::Evaluated {
        plan_id: plan.plan_id().to_string(),
        issue_id: plan.issue_id().to_string(),
        goal: plan.goal().to_string(),
        world_id: found_world_id,
        world_sha256: found_world_sha256,
        binding_matches,
        succession: succession.cloned(),
        outcome,
        admissibility,
        missing_region_facts,
        items,
        limitations,
    }
}

fn criterion_outcome(
    criterion: &AcceptanceCriterion,
    values: &BTreeMap<String, Value>,
) -> ItemOutcome {
    ItemOutcome {
        kind: ItemKind::Criterion,
        name: criterion.name.clone(),
        statement: criterion.statement.clone(),
        origin: criterion.origin,
        status: ItemStatus::of(&criterion.predicate, values),
    }
}

fn obligation_outcome(obligation: &Obligation, values: &BTreeMap<String, Value>) -> ItemOutcome {
    ItemOutcome {
        kind: ItemKind::Obligation,
        name: obligation.name.clone(),
        statement: obligation.statement.clone(),
        origin: obligation.origin,
        status: ItemStatus::of(&obligation.predicate, values),
    }
}

fn falsifier_outcome(falsifier: &Falsifier, values: &BTreeMap<String, Value>) -> ItemOutcome {
    ItemOutcome {
        kind: ItemKind::Falsifier,
        name: falsifier.name.clone(),
        statement: falsifier.statement.clone(),
        origin: falsifier.origin,
        status: ItemStatus::of(&falsifier.predicate, values),
    }
}

/// The ordering argued on [`Outcome`], in code.
fn decide(items: &[ItemOutcome]) -> Outcome {
    let of_kind = |kind: ItemKind| items.iter().filter(move |item| item.kind == kind);
    if of_kind(ItemKind::Falsifier).any(|item| item.status == ItemStatus::Met) {
        return Outcome::Falsified;
    }
    fn unevaluable(item: &ItemOutcome) -> bool {
        matches!(item.status, ItemStatus::NotEvaluable(_))
    }
    if of_kind(ItemKind::Falsifier).any(unevaluable) || of_kind(ItemKind::Criterion).any(unevaluable)
    {
        return Outcome::Underdetermined;
    }
    if of_kind(ItemKind::Criterion).any(|item| item.status == ItemStatus::Unmet) {
        return Outcome::NotMet;
    }
    Outcome::Met
}

fn decide_admissibility(items: &[ItemOutcome]) -> Admissibility {
    let obligations: Vec<&ItemOutcome> = items
        .iter()
        .filter(|item| item.kind == ItemKind::Obligation)
        .collect();
    if obligations.is_empty() {
        return Admissibility::Undeclared;
    }
    if obligations
        .iter()
        .any(|item| matches!(item.status, ItemStatus::NotEvaluable(_)))
    {
        return Admissibility::Undetermined;
    }
    if obligations
        .iter()
        .any(|item| item.status == ItemStatus::Unmet)
    {
        return Admissibility::Violated;
    }
    Admissibility::Held
}

const STALE_LIMITATION: &str =
    "No criterion, obligation or falsifier was evaluated. The world offered is not the world this \
     plan was bound to, and a verdict computed against a different world is not a verdict about \
     this plan.";

const REPORT_LIMITATIONS: [&str; 3] = [
    "This report states which of the plan's declared criteria held in the world it was checked \
     against. It does not state that the issue is resolved.",
    "Obligations are not in the outcome. They ask whether the change was admissible to make, which \
     is a question about the moment before the change; this verifier sees one world, so it checks \
     them retrospectively. Read the admissibility value separately.",
    "Criteria were evaluated against every fact in the world, not against a recompiled evidence \
     region, so a criterion may read a variable the region compiler would have judged irrelevant \
     to this issue. No budget was applied and nothing was executed.",
];

impl AcceptanceReport {
    /// The wire form.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "schema_version".to_string(),
            Value::String(REPORT_SCHEMA_VERSION.to_string()),
        );
        match self {
            AcceptanceReport::Stale {
                plan_id,
                issue_id,
                expected_world_id,
                found_world_id,
                expected_world_sha256,
                found_world_sha256,
                limitations,
            } => {
                map.insert("verdict".to_string(), Value::String("stale".to_string()));
                map.insert("plan_id".to_string(), Value::String(plan_id.clone()));
                map.insert("issue_id".to_string(), Value::String(issue_id.clone()));
                map.insert(
                    "expected_world_id".to_string(),
                    Value::String(expected_world_id.clone()),
                );
                map.insert(
                    "found_world_id".to_string(),
                    Value::String(found_world_id.clone()),
                );
                map.insert(
                    "expected_world_sha256".to_string(),
                    Value::String(expected_world_sha256.clone()),
                );
                map.insert(
                    "found_world_sha256".to_string(),
                    Value::String(found_world_sha256.clone()),
                );
                map.insert("limitations".to_string(), strings(limitations));
            }
            AcceptanceReport::Evaluated {
                plan_id,
                issue_id,
                goal,
                world_id,
                world_sha256,
                binding_matches,
                succession,
                outcome,
                admissibility,
                missing_region_facts,
                items,
                limitations,
            } => {
                map.insert(
                    "verdict".to_string(),
                    Value::String("evaluated".to_string()),
                );
                map.insert("plan_id".to_string(), Value::String(plan_id.clone()));
                map.insert("issue_id".to_string(), Value::String(issue_id.clone()));
                map.insert("goal".to_string(), Value::String(goal.clone()));
                map.insert("world_id".to_string(), Value::String(world_id.clone()));
                map.insert(
                    "world_sha256".to_string(),
                    Value::String(world_sha256.clone()),
                );
                map.insert("binding_matches".to_string(), Value::Bool(*binding_matches));
                map.insert(
                    "succession".to_string(),
                    match succession {
                        None => Value::Null,
                        Some(succession) => {
                            let mut declared = Map::new();
                            declared.insert(
                                "declared_by".to_string(),
                                Value::String(succession.declared_by.clone()),
                            );
                            declared.insert(
                                "statement".to_string(),
                                Value::String(succession.statement.clone()),
                            );
                            Value::Object(declared)
                        }
                    },
                );
                map.insert(
                    "outcome".to_string(),
                    Value::String(outcome.as_str().to_string()),
                );
                map.insert(
                    "admissibility".to_string(),
                    Value::String(admissibility.as_str().to_string()),
                );
                map.insert(
                    "missing_region_facts".to_string(),
                    strings(missing_region_facts),
                );
                map.insert(
                    "items".to_string(),
                    Value::Array(items.iter().map(item_to_json).collect()),
                );
                map.insert("limitations".to_string(), strings(limitations));
            }
        }
        Value::Object(map)
    }

    /// The strict reader. Undeclared keys are refused, and the two verdict shapes have two
    /// different declared key sets so a stale report cannot arrive wearing a verdict's fields.
    ///
    /// An evaluated report's `outcome` and `admissibility` are rederived from its own item
    /// statuses and refused when they disagree, exactly as [`RepairPlan::from_json`] rederives the
    /// plan id. Both are total functions of the item list, so there is no document a producer
    /// could legitimately emit that fails this — and without it a hand-edited report could read
    /// `"outcome": "met"` beside an item that never ran, which is the one thing this crate exists
    /// to make unsayable.
    pub fn from_json(document: &Value) -> Result<AcceptanceReport, RepairError> {
        let verdict = document
            .as_object()
            .and_then(|map| map.get("verdict"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RepairError::Document(
                    "acceptance report declares no string \"verdict\"".to_string(),
                )
            })?;
        match verdict {
            "stale" => {
                let map = strict_object(
                    document,
                    "stale acceptance report",
                    &[
                        "schema_version",
                        "verdict",
                        "plan_id",
                        "issue_id",
                        "expected_world_id",
                        "found_world_id",
                        "expected_world_sha256",
                        "found_world_sha256",
                        "limitations",
                    ],
                )?;
                check_version(map)?;
                let what = "stale acceptance report";
                Ok(AcceptanceReport::Stale {
                    plan_id: required_str(map, what, "plan_id")?,
                    issue_id: required_str(map, what, "issue_id")?,
                    expected_world_id: required_str(map, what, "expected_world_id")?,
                    found_world_id: required_str(map, what, "found_world_id")?,
                    expected_world_sha256: required_str(map, what, "expected_world_sha256")?,
                    found_world_sha256: required_str(map, what, "found_world_sha256")?,
                    limitations: string_array(map, "limitations")?,
                })
            }
            "evaluated" => {
                let map = strict_object(
                    document,
                    "acceptance report",
                    &[
                        "schema_version",
                        "verdict",
                        "plan_id",
                        "issue_id",
                        "goal",
                        "world_id",
                        "world_sha256",
                        "binding_matches",
                        "succession",
                        "outcome",
                        "admissibility",
                        "missing_region_facts",
                        "items",
                        "limitations",
                    ],
                )?;
                check_version(map)?;
                let what = "acceptance report";
                let succession = match map.get("succession") {
                    None | Some(Value::Null) => None,
                    Some(value) => {
                        let declared =
                            strict_object(value, "succession", &["declared_by", "statement"])?;
                        Some(Succession::declare(
                            required_str(declared, "succession", "declared_by")?,
                            required_str(declared, "succession", "statement")?,
                        )?)
                    }
                };
                let items: Vec<ItemOutcome> = map
                    .get("items")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        RepairError::Document(
                            "acceptance report needs an array \"items\"".to_string(),
                        )
                    })?
                    .iter()
                    .map(item_from_json)
                    .collect::<Result<Vec<_>, _>>()?;
                let outcome = Outcome::parse(&required_str(map, what, "outcome")?)?;
                let admissibility =
                    Admissibility::parse(&required_str(map, what, "admissibility")?)?;
                // Both aggregates are total functions of the item list, so a document may not
                // declare one the items do not produce — for the reason `RepairPlan::from_json`
                // rederives the plan id. The failure this refuses is the one the crate exists to
                // refuse: a report reading `"outcome": "met"` beside an item that could not run,
                // where an aggregate hides which check never happened.
                if outcome != decide(&items) {
                    return Err(RepairError::Document(format!(
                        "the report declares outcome {:?} but its own item statuses produce {:?}; \
                         the outcome is derived from the items, so a declared one they do not \
                         produce is an aggregate that hides what the items say",
                        outcome.as_str(),
                        decide(&items).as_str()
                    )));
                }
                if admissibility != decide_admissibility(&items) {
                    return Err(RepairError::Document(format!(
                        "the report declares admissibility {:?} but its own obligation statuses \
                         produce {:?}",
                        admissibility.as_str(),
                        decide_admissibility(&items).as_str()
                    )));
                }
                Ok(AcceptanceReport::Evaluated {
                    plan_id: required_str(map, what, "plan_id")?,
                    issue_id: required_str(map, what, "issue_id")?,
                    goal: required_str(map, what, "goal")?,
                    world_id: required_str(map, what, "world_id")?,
                    world_sha256: required_str(map, what, "world_sha256")?,
                    binding_matches: map
                        .get("binding_matches")
                        .and_then(Value::as_bool)
                        .ok_or_else(|| {
                            RepairError::Document(
                                "acceptance report needs a boolean \"binding_matches\"".to_string(),
                            )
                        })?,
                    succession,
                    outcome,
                    admissibility,
                    missing_region_facts: string_array(map, "missing_region_facts")?,
                    items,
                    limitations: string_array(map, "limitations")?,
                })
            }
            other => Err(RepairError::Document(format!(
                "acceptance report verdict must be \"stale\" or \"evaluated\", found {other:?}"
            ))),
        }
    }
}

fn check_version(map: &Map<String, Value>) -> Result<(), RepairError> {
    let version = required_str(map, "acceptance report", "schema_version")?;
    if version != REPORT_SCHEMA_VERSION {
        return Err(RepairError::Document(format!(
            "acceptance report declares schema_version {version:?}, expected \
             {REPORT_SCHEMA_VERSION:?}"
        )));
    }
    Ok(())
}

fn item_to_json(item: &ItemOutcome) -> Value {
    let mut map = Map::new();
    map.insert(
        "kind".to_string(),
        Value::String(item.kind.as_str().to_string()),
    );
    map.insert("name".to_string(), Value::String(item.name.clone()));
    map.insert(
        "statement".to_string(),
        Value::String(item.statement.clone()),
    );
    map.insert(
        "origin".to_string(),
        Value::String(item.origin.as_str().to_string()),
    );
    map.insert(
        "status".to_string(),
        Value::String(item.status.as_str().to_string()),
    );
    map.insert(
        "obstruction".to_string(),
        match item.status.obstruction() {
            None => Value::Null,
            Some(obstruction) => {
                let mut blocked = Map::new();
                blocked.insert(
                    "variable".to_string(),
                    Value::String(obstruction.variable.clone()),
                );
                blocked.insert(
                    "reason".to_string(),
                    Value::String(obstruction.reason.clone()),
                );
                Value::Object(blocked)
            }
        },
    );
    Value::Object(map)
}

fn item_from_json(document: &Value) -> Result<ItemOutcome, RepairError> {
    let map = strict_object(
        document,
        "reported item",
        &["kind", "name", "statement", "origin", "status", "obstruction"],
    )?;
    let what = "reported item";
    let status_text = required_str(map, what, "status")?;
    let obstruction = match map.get("obstruction") {
        None | Some(Value::Null) => None,
        Some(value) => {
            let blocked = strict_object(value, "obstruction", &["variable", "reason"])?;
            Some(Obstruction {
                variable: required_str(blocked, "obstruction", "variable")?,
                reason: required_str(blocked, "obstruction", "reason")?,
            })
        }
    };
    let status = match (status_text.as_str(), obstruction) {
        ("met", None) => ItemStatus::Met,
        ("unmet", None) => ItemStatus::Unmet,
        ("not_evaluable", Some(obstruction)) => ItemStatus::NotEvaluable(obstruction),
        ("not_evaluable", None) => {
            return Err(RepairError::Document(
                "an item reported \"not_evaluable\" without naming the obstruction that stopped \
                 it; the whole point of the third value is that it says what blocked the check"
                    .to_string(),
            ))
        }
        (other, Some(_)) => {
            return Err(RepairError::Document(format!(
                "an item reported status {other:?} while carrying an obstruction; only \
                 \"not_evaluable\" may"
            )))
        }
        (other, None) => {
            return Err(RepairError::Document(format!(
                "unknown item status {other:?}"
            )))
        }
    };
    Ok(ItemOutcome {
        kind: ItemKind::parse(&required_str(map, what, "kind")?)?,
        name: required_str(map, what, "name")?,
        statement: required_str(map, what, "statement")?,
        origin: Origin::parse(&required_str(map, what, "origin")?)?,
        status,
    })
}

fn strings(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: ItemKind, name: &str, status: ItemStatus) -> ItemOutcome {
        ItemOutcome {
            kind,
            name: name.to_string(),
            statement: "synthetic".to_string(),
            origin: Origin::Declared,
            status,
        }
    }

    fn blocked() -> ItemStatus {
        ItemStatus::NotEvaluable(Obstruction {
            variable: "v".to_string(),
            reason: "absent from the compiled value map".to_string(),
        })
    }

    #[test]
    fn a_met_falsifier_decides_the_outcome_whatever_the_criteria_said() {
        for accompanying in [ItemStatus::Met, ItemStatus::Unmet, blocked()] {
            let items = vec![
                item(ItemKind::Criterion, "c", accompanying.clone()),
                item(ItemKind::Falsifier, "f", ItemStatus::Met),
            ];
            assert_eq!(decide(&items), Outcome::Falsified);
        }
    }

    #[test]
    fn a_falsifier_that_could_not_run_leaves_the_outcome_underdetermined_not_met() {
        let items = vec![
            item(ItemKind::Criterion, "c", ItemStatus::Met),
            item(ItemKind::Falsifier, "f", blocked()),
        ];
        assert_eq!(
            decide(&items),
            Outcome::Underdetermined,
            "nobody checked whether the plan is wrong, which is not the same as it being right"
        );
    }

    #[test]
    fn an_unevaluable_criterion_outranks_an_unmet_one() {
        let items = vec![
            item(ItemKind::Criterion, "a", ItemStatus::Unmet),
            item(ItemKind::Criterion, "b", blocked()),
            item(ItemKind::Falsifier, "f", ItemStatus::Unmet),
        ];
        assert_eq!(decide(&items), Outcome::Underdetermined);
    }

    #[test]
    fn every_criterion_met_with_no_blind_check_is_the_only_route_to_met() {
        let items = vec![
            item(ItemKind::Criterion, "a", ItemStatus::Met),
            item(ItemKind::Falsifier, "f", ItemStatus::Unmet),
        ];
        assert_eq!(decide(&items), Outcome::Met);
    }

    #[test]
    fn an_obligation_never_moves_the_achievement_outcome() {
        let met = vec![
            item(ItemKind::Criterion, "a", ItemStatus::Met),
            item(ItemKind::Falsifier, "f", ItemStatus::Unmet),
        ];
        let mut with_broken_obligation = met.clone();
        with_broken_obligation.push(item(ItemKind::Obligation, "o", ItemStatus::Unmet));
        let mut with_blind_obligation = met.clone();
        with_blind_obligation.push(item(ItemKind::Obligation, "o", blocked()));

        assert_eq!(decide(&met), Outcome::Met);
        assert_eq!(decide(&with_broken_obligation), Outcome::Met);
        assert_eq!(decide(&with_blind_obligation), Outcome::Met);
        assert_eq!(
            decide_admissibility(&with_broken_obligation),
            Admissibility::Violated
        );
        assert_eq!(
            decide_admissibility(&with_blind_obligation),
            Admissibility::Undetermined
        );
    }

    #[test]
    fn a_plan_with_no_obligation_reports_undeclared_rather_than_held() {
        let items = vec![item(ItemKind::Criterion, "a", ItemStatus::Met)];
        assert_eq!(decide_admissibility(&items), Admissibility::Undeclared);
    }
}
