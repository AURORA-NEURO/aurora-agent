//! Deriving a defensible starting plan from what the scan can actually be held to.
//!
//! [`plan_for_issue`] infers three kinds of thing and refuses to invent a fourth.
//!
//! # What it derives, and why each is defensible
//!
//! **One criterion per fired release check that reads a variable in the issue's region.** If the
//! `project-release-readiness` pack fired `unpinned_dependency` and `unpinned_dependencies` is in
//! the evidence region compiled for this issue, then "that check no longer fires" is something the
//! same scan can check later, over the same variable, with no new machinery. Checks that did *not*
//! fire produce nothing: asking a repair to keep clean something already clean would be a
//! criterion nobody declared.
//!
//! The predicate is the check's **own predicate wrapped in [`Predicate::Not`]**, not a copy of the
//! check with an "expected outcome" flag flipped. This is the load-bearing choice in the module.
//! `Predicate::Not` evaluates as `Ok(!inner?)`, so an unevaluable limb propagates: a check that
//! could not run yields a criterion that could not run. An "expected outcome" encoding would have
//! to decide what an unevaluable check means, and the tempting default — treat "did not fire" as
//! "passed" — is precisely the lie this crate exists to refuse. Negation keeps three values three.
//!
//! **One criterion per component the issue declares, asserting it still exists.** A component
//! whose inventory variable has vanished is a real failure mode, not a pedantic one: the cheapest
//! way to stop a static check firing over a file is to delete the file. [`Predicate::Exists`] is
//! one of the two total predicates in the language, so a vanished component is a determinate
//! `Unmet`, never an `NotEvaluable` a reader could shrug at.
//!
//! **One falsifier, over the decisive variables.** The decisive set is every variable the derived
//! criteria read. If any of them is absent from the world being verified, the plan is not merely
//! uncheckable — its premise, that this region is the evidence for this issue, is false about that
//! world. See [`crate::Outcome`] for why that outranks `Underdetermined`.
//!
//! # What a good plan needs that this generator cannot produce
//!
//! Stated here and carried into every generated plan's `limitations`, because a generator that
//! quietly produces a thin plan and calls it a plan is the same defect as a checker that quietly
//! calls an unevaluable criterion met.
//!
//! * **Nothing derived from the issue text.** The generator never reads the title or body as
//!   language. It cannot produce a criterion about the behaviour the issue actually describes, and
//!   every derived criterion is a proxy for something the release pack could see rather than for
//!   what the issue means.
//! * **No criterion that the reported symptom is gone.** A repair satisfying every derived
//!   criterion may leave the behaviour exactly as it was.
//! * **No regression guard.** Checks the pack currently judges clean produce no criterion, so this
//!   plan cannot notice a repair that breaks one of them.
//! * **A derived falsifier with no teeth, for some issues.** The falsifier watches the decisive
//!   variables for absence, and an issue that declares no component has only aggregate variables
//!   in its decisive set — variables the world assembler emits unconditionally, empty or not. Such
//!   a falsifier can essentially never hold. The plan is still refused if it ends up with no
//!   falsifier at all, but "has a falsifier" and "has a falsifier that could realistically fire"
//!   are different states, and a plan in the second-weakest one says so in its own limitations
//!   rather than passing the admissibility gate quietly.
//! * **No obligation is ever derived.** Whether a change is admissible to make is a judgement about
//!   process — review, test coverage, blast radius — and the scan sees none of it. Every obligation
//!   in a plan is a human's, and a plan with no obligations declares no prerequisites rather than
//!   declaring that none are needed.
//! * **No cost, effort, ordering or risk.** This is not a work plan and proposes no steps.

use crate::plan::{
    AcceptanceCriterion, EvidenceBinding, Falsifier, Obligation, Origin, RepairPlan,
    RepairPlanDraft, CRITERIA_ARE_NOT_PROOF,
};
use crate::verify::world_value_map;
use crate::RepairError;
use bioprism_domain::{DomainPack, Predicate};
use bioprism_project::scan::component_slug;
use bioprism_project::AGGREGATE_VARIABLES;
use bioprism_section::ContextCertificate;
use bioprism_world::World;
use serde_json::Value;
use std::collections::BTreeSet;

/// The name of the single derived falsifier.
pub const REGION_EVIDENCE_REMOVED: &str = "region_evidence_removed";

/// An item a caller asserts, before the generator stamps its origin.
///
/// It has no `origin` field on purpose. The generator is the only thing that can mint
/// [`Origin::Derived`], so no caller-supplied item can be recorded as an inference and no
/// inference can be recorded as a caller's declaration. That is a type-level guarantee rather than
/// a convention someone has to remember.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredItem {
    pub name: String,
    pub statement: String,
    pub predicate: Predicate,
    /// Used only for criteria; obligations and falsifiers do not carry one on the wire.
    pub rationale: String,
}

impl DeclaredItem {
    pub fn new(
        name: impl Into<String>,
        statement: impl Into<String>,
        predicate: Predicate,
    ) -> DeclaredItem {
        DeclaredItem {
            name: name.into(),
            statement: statement.into(),
            predicate,
            rationale: String::new(),
        }
    }

    pub fn with_rationale(mut self, rationale: impl Into<String>) -> DeclaredItem {
        self.rationale = rationale.into();
        self
    }
}

/// What a caller adds to the derived plan.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanOptions {
    pub declared_criteria: Vec<DeclaredItem>,
    pub declared_obligations: Vec<DeclaredItem>,
    pub declared_falsifiers: Vec<DeclaredItem>,
    /// Appended after the generator's own limitations, never replacing them.
    pub limitations: Vec<String>,
}

/// Derives a repair plan for one issue from the world it lives in and the region compiled for it.
///
/// `region` is the [`ContextCertificate`] of a compiled issue query — this crate takes the
/// certificate rather than the compiler's output so it does not link the engine, the same reason
/// `bioprism-section` depends on neither `world` nor `fiber`.
///
/// Fails rather than guessing when the certificate is not about this world: a plan bound to a
/// region that was compiled from something else would be bound to nothing.
pub fn plan_for_issue(
    world: &World,
    pack: &DomainPack,
    issue_id: &str,
    region: &ContextCertificate,
    options: &PlanOptions,
) -> Result<RepairPlan, RepairError> {
    let world_sha256 = world.content_hash().as_str().to_string();
    if region.world_id != world.world_id.as_str() {
        return Err(RepairError::RegionWorldMismatch {
            expected: world.world_id.as_str().to_string(),
            found: region.world_id.clone(),
        });
    }
    if region.source_hashes.world_sha256 != world_sha256 {
        return Err(RepairError::RegionWorldMismatch {
            expected: world_sha256,
            found: region.source_hashes.world_sha256.clone(),
        });
    }

    let issue_variable = format!("issue_{issue_id}_record");
    let issue_fact = world
        .fact_providing(&issue_variable)
        .ok_or_else(|| RepairError::UnknownIssue {
            issue_id: issue_id.to_string(),
            variable: issue_variable.clone(),
        })?;
    let goal = issue_fact
        .value
        .get("title")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RepairError::MalformedIssueFact(format!(
                "{issue_variable:?} carries no string \"title\" to use as the goal"
            ))
        })?
        .to_string();

    let mut region_fact_ids = region.selected_facts.clone();
    region_fact_ids.sort();
    region_fact_ids.dedup();
    let mut region_variables: BTreeSet<String> = BTreeSet::new();
    for id in &region_fact_ids {
        let fact = world
            .fact(id)
            .ok_or_else(|| RepairError::RegionFactUnknown {
                fact_id: id.clone(),
            })?;
        region_variables.insert(fact.provides.as_str().to_string());
    }

    let values = world_value_map(world);
    let mut criteria: Vec<AcceptanceCriterion> = Vec::new();
    let mut limitations: Vec<String> = vec![CRITERIA_ARE_NOT_PROOF.to_string()];
    limitations.extend(STANDING_LIMITATIONS.iter().map(|line| line.to_string()));

    let mut fired = 0usize;
    for check in pack.oracle().checks() {
        let read = check.when.variables();
        if read.is_disjoint(&region_variables) {
            continue;
        }
        match check.when.evaluate(&values) {
            Ok(true) => {
                fired += 1;
                criteria.push(AcceptanceCriterion {
                    name: format!("check_cleared:{}", check.name),
                    statement: format!(
                        "The release check {:?} no longer fires over this issue's evidence \
                         region. What that check judges, in its own words: {}",
                        check.name, check.description
                    ),
                    predicate: Predicate::Not {
                        predicate: Box::new(check.when.clone()),
                    },
                    rationale: format!(
                        "Derived: {:?} fired when this plan was made, and it reads a variable in \
                         the region compiled for this issue, so clearing it is something the same \
                         static scan can be held to later. The predicate is the check's own \
                         predicate under \"not\", so a check that cannot run yields a criterion \
                         that cannot run rather than one that counts as cleared.",
                        check.name
                    ),
                    origin: Origin::Derived,
                });
            }
            Ok(false) => {}
            Err(obstruction) => limitations.push(format!(
                "The release check {:?} could not be evaluated when this plan was made: variable \
                 {:?} {}. No criterion was derived from it, and whether it fires is unknown, not \
                 false.",
                check.name, obstruction.variable, obstruction.reason
            )),
        }
    }
    if fired == 0 {
        limitations.push(
            "No release check fired over this issue's evidence region when the plan was made, so \
             the derived criteria rest on component presence alone."
                .to_string(),
        );
    }

    for component in issue_fact
        .value
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RepairError::MalformedIssueFact(format!(
                "{issue_variable:?} carries no array \"components\""
            ))
        })?
    {
        let display = component.as_str().ok_or_else(|| {
            RepairError::MalformedIssueFact(format!(
                "{issue_variable:?} carries a non-string component entry"
            ))
        })?;
        criteria.push(AcceptanceCriterion {
            name: format!("component_present:{display}"),
            statement: format!(
                "The component {display:?}, which this issue declares, still exists in the \
                 scanned tree."
            ),
            predicate: Predicate::Exists {
                variable: format!("component_{}_inventory", component_slug(display)),
            },
            rationale: format!(
                "Derived: an issue is not repaired by deleting the component it names, and the \
                 cheapest way to stop a static check firing over {display:?} is to remove it. \
                 \"exists\" is total, so a vanished component is a determinate failure rather \
                 than an unevaluable one."
            ),
            origin: Origin::Derived,
        });
    }

    if let Some(unresolved) = issue_fact
        .value
        .get("unresolved_components")
        .and_then(Value::as_array)
    {
        if !unresolved.is_empty() {
            limitations.push(format!(
                "The issue declares components the scan could not resolve to anything in the \
                 tree ({}); no criterion covers them.",
                unresolved
                    .iter()
                    .map(|item| item.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    let decisive: BTreeSet<String> = criteria
        .iter()
        .flat_map(|criterion| criterion.predicate.variables())
        .collect();
    let mut falsifiers: Vec<Falsifier> = Vec::new();
    if decisive.is_empty() {
        limitations.push(
            "Nothing could be derived for this issue: no release check fired over its region and \
             it declares no resolvable component, so this plan carries no derived falsifier."
                .to_string(),
        );
    } else {
        falsifiers.push(Falsifier {
            name: REGION_EVIDENCE_REMOVED.to_string(),
            statement: format!(
                "A variable this plan's derived criteria reason from is absent from the world \
                 being verified: {}. The evidence region the plan was made from no longer exists, \
                 so the plan is about a tree that is gone.",
                decisive
                    .iter()
                    .map(|variable| format!("{variable:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            predicate: Predicate::AnyOf {
                predicates: decisive
                    .iter()
                    .map(|variable| Predicate::Missing {
                        variable: variable.clone(),
                    })
                    .collect(),
            },
            origin: Origin::Derived,
        });
        if decisive
            .iter()
            .all(|variable| AGGREGATE_VARIABLES.contains(&variable.as_str()))
        {
            limitations.push(
                "Every variable this plan's derived falsifier watches is one the world assembler \
                 emits unconditionally, so the derived falsifier is very unlikely ever to hold. \
                 Treat this plan as effectively carrying no derived falsifier and declare one \
                 with real teeth."
                    .to_string(),
            );
        }
    }

    criteria.extend(options.declared_criteria.iter().map(|item| {
        AcceptanceCriterion {
            name: item.name.clone(),
            statement: item.statement.clone(),
            predicate: item.predicate.clone(),
            rationale: item.rationale.clone(),
            origin: Origin::Declared,
        }
    }));
    let mut obligations: Vec<Obligation> = options
        .declared_obligations
        .iter()
        .map(|item| Obligation {
            name: item.name.clone(),
            statement: item.statement.clone(),
            predicate: item.predicate.clone(),
            origin: Origin::Declared,
        })
        .collect();
    falsifiers.extend(options.declared_falsifiers.iter().map(|item| Falsifier {
        name: item.name.clone(),
        statement: item.statement.clone(),
        predicate: item.predicate.clone(),
        origin: Origin::Declared,
    }));

    criteria.sort_by(|left, right| left.name.cmp(&right.name));
    obligations.sort_by(|left, right| left.name.cmp(&right.name));
    falsifiers.sort_by(|left, right| left.name.cmp(&right.name));

    if obligations.is_empty() {
        limitations.push(
            "This plan declares no obligations. That is a statement that no prerequisite was \
             declared, not that none is needed: nothing here derives obligations."
                .to_string(),
        );
    }
    limitations.extend(options.limitations.iter().cloned());

    RepairPlan::admit(RepairPlanDraft {
        issue_id: issue_id.to_string(),
        goal,
        evidence_binding: EvidenceBinding {
            world_id: world.world_id.as_str().to_string(),
            world_sha256,
            region_fact_ids,
            query_sha256: region.source_hashes.query_sha256.clone(),
        },
        criteria,
        obligations,
        falsifiers,
        limitations,
    })
}

/// The gaps every generated plan carries, whatever the issue.
const STANDING_LIMITATIONS: [&str; 5] = [
    "No criterion here was derived from the issue's text: this generator does not read prose. \
     Every derived criterion is a proxy for something the release pack could see, not for what \
     the issue means.",
    "No criterion asserts that the reported behaviour changed. A repair satisfying every derived \
     criterion may leave the symptom exactly as it was.",
    "Nothing is executed. A criterion over a test count is a claim about the static scan, and a \
     counted test is not a passing test.",
    "Checks the pack currently judges clean produced no criterion, so this plan cannot notice a \
     repair that breaks one of them.",
    "No cost, effort, ordering or risk is estimated. This is not a work plan and proposes no \
     steps.",
];
