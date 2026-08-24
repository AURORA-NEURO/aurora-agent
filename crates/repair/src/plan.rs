//! The repair plan document: what a human declares would count as having fixed an issue.
//!
//! A plan is three lists of named, predicate-backed items and one binding to the evidence it was
//! made from. The three lists are deliberately three types rather than one:
//!
//! * an [`AcceptanceCriterion`] is about whether the change **achieved** the goal;
//! * an [`Obligation`] is about whether the change was **admissible to make** — a prerequisite the
//!   author declares must hold before touching the code;
//! * a [`Falsifier`] is an observation that would prove the **plan itself wrong**.
//!
//! The distinction between the first two survives scrutiny even though this crate evaluates both
//! against one world. A criterion and an obligation answer different questions about different
//! moments, and collapsing them would force one of two lies: either an unmet prerequisite would
//! be reported as a failure to achieve the goal (it is not — the goal may well have been reached
//! by a change that should not have been made), or a met prerequisite would inflate the count of
//! criteria that held. What this crate genuinely cannot do is observe the "before" moment, so an
//! obligation checked here is checked *retrospectively*, and [`crate::verify()`] keeps obligations
//! out of the achievement outcome and reports them on their own axis. That limitation is stated
//! on every report rather than papered over by merging the types.
//!
//! # Why an empty falsifier list is refused
//!
//! `bioprism_foundation::contract::FalsifiableContract::admit` refuses a contract whose falsifier
//! list is empty, and its module documentation gives the reason in the blueprint's own words: a
//! contract with no falsifiers "is not a strict contract with no falsifiers yet — it is a
//! benchmark that cannot be failed". A repair plan is the same object wearing different clothes.
//! A plan that declares only criteria declares only ways to succeed; nothing in it could ever come
//! back and say *this plan was the wrong plan*. [`RepairPlan::admit`] therefore refuses it, and
//! [`RepairPlan`] has private fields with `admit` as the only constructor, so the refusal cannot
//! be routed around by building the checked type directly — the same gating `FalsifiableContract`
//! uses.
//!
//! # Why the plan is bound to its evidence
//!
//! [`EvidenceBinding`] records the world id, the world digest, the compiled region's fact ids and
//! the query digest. A verdict computed against a different world is not a verdict about this
//! plan, and the binding is what lets [`crate::verify()`] say so instead of guessing.

use crate::predicate_json::{predicate_from_json, predicate_to_json};
use crate::RepairError;
use bioprism_domain::Predicate;
use bioprism_ids::ContentHash;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// The plan document's schema version.
pub const PLAN_SCHEMA_VERSION: &str = "bioprism-repair-plan/0.1";

/// The limitation every plan must carry, verbatim.
///
/// It is a constant rather than free prose because [`RepairPlan::admit`] refuses a plan that does
/// not contain it: a plan is allowed to add limitations, never to drop this one.
pub const CRITERIA_ARE_NOT_PROOF: &str =
    "Meeting every criterion in this plan is not proof that the issue is resolved. The criteria \
     are the plan author's declaration of what would count as evidence; the gap between that \
     declaration and the issue itself belongs to the author, and no tool closes it.";

/// Who put an item in the plan.
///
/// A reader must be able to tell what the generator inferred from what a person asserted, because
/// the two carry different authority: a derived criterion is a proxy for something the release
/// pack could see, and a declared one is a claim someone is accountable for.
///
/// On the generation path this is enforced by the types rather than by care: a caller hands
/// [`crate::plan_for_issue`] a [`crate::DeclaredItem`], which has **no origin field at all**, so
/// nothing a caller supplies can arrive pre-labelled and the generator is the only thing that
/// stamps [`Origin::Derived`]. The guarantee is exactly that wide and no wider — [`RepairPlan::admit`]
/// and [`RepairPlan::from_json`] accept whatever origin a hand-built draft or a parsed document
/// carries, because a document that already exists says what it says and this crate does not get
/// to overrule its author.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// Inferred by [`crate::plan_for_issue`] from the world and the pack.
    Derived,
    /// Supplied by the caller.
    Declared,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Derived => "derived",
            Origin::Declared => "declared",
        }
    }

    pub fn parse(text: &str) -> Result<Origin, RepairError> {
        match text {
            "derived" => Ok(Origin::Derived),
            "declared" => Ok(Origin::Declared),
            other => Err(RepairError::Document(format!(
                "origin must be \"derived\" or \"declared\", found {other:?}"
            ))),
        }
    }
}

/// One thing that must hold for the goal to count as achieved.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptanceCriterion {
    pub name: String,
    /// Prose: what a human would check, in words that survive being quoted alone.
    pub statement: String,
    /// The machine-checkable form, evaluated against a future scan's value map under
    /// `bioprism_domain::Predicate`'s strong three-valued rules.
    pub predicate: Predicate,
    /// Why this criterion is in the plan at all.
    pub rationale: String,
    pub origin: Origin,
}

/// A prerequisite the plan author declares must hold before the change may be made.
#[derive(Debug, Clone, PartialEq)]
pub struct Obligation {
    pub name: String,
    pub statement: String,
    pub predicate: Predicate,
    pub origin: Origin,
}

/// An observation that would prove the plan wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct Falsifier {
    pub name: String,
    pub statement: String,
    pub predicate: Predicate,
    pub origin: Origin,
}

/// The evidence region a plan was made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceBinding {
    pub world_id: String,
    /// `bioprism_world::World::content_hash` of the world document the plan was made from.
    pub world_sha256: String,
    /// The compiled region's fact ids, ascending and without duplicates.
    pub region_fact_ids: Vec<String>,
    /// The digest of the query that compiled the region, from the certificate's `source_hashes`.
    pub query_sha256: String,
}

/// The unchecked form of a plan, as authored or parsed.
///
/// Kept separate from [`RepairPlan`] so the admissibility gate cannot be skipped, exactly as
/// `bioprism_foundation::contract::ContractDraft` is kept separate from its checked contract.
#[derive(Debug, Clone, PartialEq)]
pub struct RepairPlanDraft {
    pub issue_id: String,
    /// The issue title, verbatim. Never a paraphrase: the plan does not get to restate the goal.
    pub goal: String,
    pub evidence_binding: EvidenceBinding,
    pub criteria: Vec<AcceptanceCriterion>,
    pub obligations: Vec<Obligation>,
    pub falsifiers: Vec<Falsifier>,
    pub limitations: Vec<String>,
}

/// A plan that has passed admissibility, carrying its content-derived id.
#[derive(Debug, Clone, PartialEq)]
pub struct RepairPlan {
    draft: RepairPlanDraft,
    plan_id: String,
}

impl RepairPlan {
    /// The admissibility gate.
    ///
    /// Each refusal names the specific defect rather than reporting "invalid plan", because the
    /// person who has to fix it is the plan's author.
    ///
    /// Duplicate names are refused across criteria, obligations *and* falsifiers together, not
    /// within each list. The acceptance report is one list of named statuses, so two items sharing
    /// a name would make a reader unable to say which one could not run — and "which criterion was
    /// not evaluated" is the exact question this crate exists to answer.
    pub fn admit(draft: RepairPlanDraft) -> Result<RepairPlan, RepairError> {
        let issue = draft.issue_id.clone();
        if draft.issue_id.trim().is_empty() {
            return Err(RepairError::EmptyField {
                what: "issue_id".into(),
            });
        }
        if draft.goal.trim().is_empty() {
            return Err(RepairError::EmptyField {
                what: "goal".into(),
            });
        }
        if draft.criteria.is_empty() {
            return Err(RepairError::NoCriterion { issue });
        }
        if draft.falsifiers.is_empty() {
            return Err(RepairError::NoFalsifier { issue });
        }
        if !draft
            .limitations
            .iter()
            .any(|line| line == CRITERIA_ARE_NOT_PROOF)
        {
            return Err(RepairError::MissingMandatoryLimitation);
        }

        check_binding(&draft.evidence_binding)?;

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (name, statement) in draft.item_names_and_statements() {
            if name.trim().is_empty() {
                return Err(RepairError::EmptyField {
                    what: "item name".into(),
                });
            }
            if statement.trim().is_empty() {
                return Err(RepairError::EmptyField {
                    what: format!("statement on item {name:?}"),
                });
            }
            if !seen.insert(name) {
                return Err(RepairError::DuplicateItemName {
                    name: name.to_string(),
                });
            }
        }

        let plan_id = derive_plan_id(&draft)?;
        Ok(RepairPlan { draft, plan_id })
    }

    /// `repair-<issue id>-<first twelve hex digits of the plan body's digest>`.
    ///
    /// Content-derived so two plans made from the same world and options carry the same id, and a
    /// plan whose body was edited after the fact cannot keep it: [`RepairPlan::from_json`] rederives
    /// the id and refuses a document whose declared id disagrees.
    pub fn plan_id(&self) -> &str {
        &self.plan_id
    }

    pub fn issue_id(&self) -> &str {
        &self.draft.issue_id
    }

    pub fn goal(&self) -> &str {
        &self.draft.goal
    }

    pub fn evidence_binding(&self) -> &EvidenceBinding {
        &self.draft.evidence_binding
    }

    pub fn criteria(&self) -> &[AcceptanceCriterion] {
        &self.draft.criteria
    }

    pub fn obligations(&self) -> &[Obligation] {
        &self.draft.obligations
    }

    pub fn falsifiers(&self) -> &[Falsifier] {
        &self.draft.falsifiers
    }

    pub fn limitations(&self) -> &[String] {
        &self.draft.limitations
    }

    /// The wire form. Emitted field by field rather than through `serde`'s derive because
    /// `bioprism_domain::Predicate` has a strict hand-written reader and no `Serialize`, and a
    /// second predicate encoding would be a second predicate language.
    pub fn to_json(&self) -> Result<Value, RepairError> {
        let mut map = plan_body(&self.draft)?;
        map.insert(
            "plan_id".to_string(),
            Value::String(self.plan_id.clone()),
        );
        Ok(Value::Object(map))
    }

    /// The strict reader: undeclared keys are refused, and so is a declared `plan_id` that the
    /// body does not hash to.
    pub fn from_json(document: &Value) -> Result<RepairPlan, RepairError> {
        let map = strict_object(
            document,
            "repair plan",
            &[
                "schema_version",
                "plan_id",
                "issue_id",
                "goal",
                "evidence_binding",
                "criteria",
                "obligations",
                "falsifiers",
                "limitations",
            ],
        )?;
        let version = required_str(map, "repair plan", "schema_version")?;
        if version != PLAN_SCHEMA_VERSION {
            return Err(RepairError::Document(format!(
                "repair plan declares schema_version {version:?}, expected \
                 {PLAN_SCHEMA_VERSION:?}"
            )));
        }
        let declared_id = required_str(map, "repair plan", "plan_id")?;

        let draft = RepairPlanDraft {
            issue_id: required_str(map, "repair plan", "issue_id")?,
            goal: required_str(map, "repair plan", "goal")?,
            evidence_binding: binding_from_json(map.get("evidence_binding").ok_or_else(|| {
                RepairError::Document("repair plan declares no \"evidence_binding\"".into())
            })?)?,
            criteria: array(map, "criteria")?
                .iter()
                .map(criterion_from_json)
                .collect::<Result<Vec<_>, _>>()?,
            obligations: array(map, "obligations")?
                .iter()
                .map(obligation_from_json)
                .collect::<Result<Vec<_>, _>>()?,
            falsifiers: array(map, "falsifiers")?
                .iter()
                .map(falsifier_from_json)
                .collect::<Result<Vec<_>, _>>()?,
            limitations: string_array(map, "limitations")?,
        };

        let plan = RepairPlan::admit(draft)?;
        if plan.plan_id != declared_id {
            return Err(RepairError::PlanIdMismatch {
                declared: declared_id,
                derived: plan.plan_id,
            });
        }
        Ok(plan)
    }
}

impl RepairPlanDraft {
    fn item_names_and_statements(&self) -> Vec<(&str, &str)> {
        let criteria = self
            .criteria
            .iter()
            .map(|item| (item.name.as_str(), item.statement.as_str()));
        let obligations = self
            .obligations
            .iter()
            .map(|item| (item.name.as_str(), item.statement.as_str()));
        let falsifiers = self
            .falsifiers
            .iter()
            .map(|item| (item.name.as_str(), item.statement.as_str()));
        criteria.chain(obligations).chain(falsifiers).collect()
    }
}

/// The binding's own admissibility. A digest that is not a digest, or a region listed in an order
/// two callers would not agree on, would both make the plan id meaningless.
fn check_binding(binding: &EvidenceBinding) -> Result<(), RepairError> {
    if binding.world_id.trim().is_empty() {
        return Err(RepairError::EmptyField {
            what: "evidence_binding.world_id".into(),
        });
    }
    for (field, digest) in [
        ("world_sha256", &binding.world_sha256),
        ("query_sha256", &binding.query_sha256),
    ] {
        ContentHash::parse(digest.clone()).map_err(|_| {
            RepairError::Document(format!(
                "evidence_binding.{field} is not a sha256 digest: {digest:?}"
            ))
        })?;
    }
    let sorted_unique = binding
        .region_fact_ids
        .windows(2)
        .all(|pair| pair[0] < pair[1]);
    if !sorted_unique {
        return Err(RepairError::RegionFactIds(
            "region_fact_ids must be ascending and free of duplicates so two callers holding the \
             same region derive the same plan id"
                .into(),
        ));
    }
    Ok(())
}

fn derive_plan_id(draft: &RepairPlanDraft) -> Result<String, RepairError> {
    let body = Value::Object(plan_body(draft)?);
    let digest = ContentHash::of_value(&body)?;
    Ok(format!(
        "repair-{}-{}",
        draft.issue_id,
        &digest.as_str()[..12]
    ))
}

/// The plan document without its `plan_id`, which is what the id is taken over.
fn plan_body(draft: &RepairPlanDraft) -> Result<Map<String, Value>, RepairError> {
    let mut map = Map::new();
    map.insert(
        "schema_version".to_string(),
        Value::String(PLAN_SCHEMA_VERSION.to_string()),
    );
    map.insert(
        "issue_id".to_string(),
        Value::String(draft.issue_id.clone()),
    );
    map.insert("goal".to_string(), Value::String(draft.goal.clone()));
    map.insert(
        "evidence_binding".to_string(),
        binding_to_json(&draft.evidence_binding),
    );
    map.insert(
        "criteria".to_string(),
        Value::Array(
            draft
                .criteria
                .iter()
                .map(criterion_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "obligations".to_string(),
        Value::Array(
            draft
                .obligations
                .iter()
                .map(obligation_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "falsifiers".to_string(),
        Value::Array(
            draft
                .falsifiers
                .iter()
                .map(falsifier_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "limitations".to_string(),
        Value::Array(
            draft
                .limitations
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    Ok(map)
}

fn binding_to_json(binding: &EvidenceBinding) -> Value {
    let mut map = Map::new();
    map.insert(
        "world_id".to_string(),
        Value::String(binding.world_id.clone()),
    );
    map.insert(
        "world_sha256".to_string(),
        Value::String(binding.world_sha256.clone()),
    );
    map.insert(
        "region_fact_ids".to_string(),
        Value::Array(
            binding
                .region_fact_ids
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    map.insert(
        "query_sha256".to_string(),
        Value::String(binding.query_sha256.clone()),
    );
    Value::Object(map)
}

fn binding_from_json(document: &Value) -> Result<EvidenceBinding, RepairError> {
    let map = strict_object(
        document,
        "evidence_binding",
        &[
            "world_id",
            "world_sha256",
            "region_fact_ids",
            "query_sha256",
        ],
    )?;
    Ok(EvidenceBinding {
        world_id: required_str(map, "evidence_binding", "world_id")?,
        world_sha256: required_str(map, "evidence_binding", "world_sha256")?,
        region_fact_ids: string_array(map, "region_fact_ids")?,
        query_sha256: required_str(map, "evidence_binding", "query_sha256")?,
    })
}

fn criterion_to_json(item: &AcceptanceCriterion) -> Result<Value, RepairError> {
    let mut map = item_map(&item.name, &item.statement, &item.predicate, item.origin)?;
    map.insert(
        "rationale".to_string(),
        Value::String(item.rationale.clone()),
    );
    Ok(Value::Object(map))
}

fn obligation_to_json(item: &Obligation) -> Result<Value, RepairError> {
    Ok(Value::Object(item_map(
        &item.name,
        &item.statement,
        &item.predicate,
        item.origin,
    )?))
}

fn falsifier_to_json(item: &Falsifier) -> Result<Value, RepairError> {
    Ok(Value::Object(item_map(
        &item.name,
        &item.statement,
        &item.predicate,
        item.origin,
    )?))
}

fn item_map(
    name: &str,
    statement: &str,
    predicate: &Predicate,
    origin: Origin,
) -> Result<Map<String, Value>, RepairError> {
    let mut map = Map::new();
    map.insert("name".to_string(), Value::String(name.to_string()));
    map.insert(
        "statement".to_string(),
        Value::String(statement.to_string()),
    );
    map.insert("predicate".to_string(), predicate_to_json(predicate)?);
    map.insert(
        "origin".to_string(),
        Value::String(origin.as_str().to_string()),
    );
    Ok(map)
}

struct ParsedItem {
    name: String,
    statement: String,
    predicate: Predicate,
    origin: Origin,
}

fn item_from_json(
    document: &Value,
    what: &str,
    declared: &[&str],
) -> Result<ParsedItem, RepairError> {
    let map = strict_object(document, what, declared)?;
    Ok(ParsedItem {
        name: required_str(map, what, "name")?,
        statement: required_str(map, what, "statement")?,
        predicate: predicate_from_json(map.get("predicate").ok_or_else(|| {
            RepairError::Document(format!("{what} declares no \"predicate\""))
        })?)?,
        origin: Origin::parse(&required_str(map, what, "origin")?)?,
    })
}

fn criterion_from_json(document: &Value) -> Result<AcceptanceCriterion, RepairError> {
    let parsed = item_from_json(
        document,
        "acceptance criterion",
        &["name", "statement", "predicate", "rationale", "origin"],
    )?;
    let map = document.as_object().expect("strict_object accepted it");
    Ok(AcceptanceCriterion {
        name: parsed.name,
        statement: parsed.statement,
        predicate: parsed.predicate,
        rationale: required_str(map, "acceptance criterion", "rationale")?,
        origin: parsed.origin,
    })
}

fn obligation_from_json(document: &Value) -> Result<Obligation, RepairError> {
    let parsed = item_from_json(
        document,
        "obligation",
        &["name", "statement", "predicate", "origin"],
    )?;
    Ok(Obligation {
        name: parsed.name,
        statement: parsed.statement,
        predicate: parsed.predicate,
        origin: parsed.origin,
    })
}

fn falsifier_from_json(document: &Value) -> Result<Falsifier, RepairError> {
    let parsed = item_from_json(
        document,
        "falsifier",
        &["name", "statement", "predicate", "origin"],
    )?;
    Ok(Falsifier {
        name: parsed.name,
        statement: parsed.statement,
        predicate: parsed.predicate,
        origin: parsed.origin,
    })
}

pub(crate) fn strict_object<'a>(
    document: &'a Value,
    what: &str,
    declared: &[&str],
) -> Result<&'a Map<String, Value>, RepairError> {
    let map = document
        .as_object()
        .ok_or_else(|| RepairError::Document(format!("{what} is not an object")))?;
    if let Some(unknown) = map.keys().find(|key| !declared.contains(&key.as_str())) {
        return Err(RepairError::Document(format!(
            "undeclared field {unknown:?} on {what}"
        )));
    }
    Ok(map)
}

pub(crate) fn required_str(
    map: &Map<String, Value>,
    what: &str,
    field: &str,
) -> Result<String, RepairError> {
    map.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| RepairError::Document(format!("{what} needs a string {field:?}")))
}

fn array<'a>(
    map: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Vec<Value>, RepairError> {
    map.get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| RepairError::Document(format!("repair plan needs an array {field:?}")))
}

pub(crate) fn string_array(
    map: &Map<String, Value>,
    field: &str,
) -> Result<Vec<String>, RepairError> {
    map.get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| RepairError::Document(format!("needs an array {field:?}")))?
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| RepairError::Document(format!("{field:?} carries a non-string entry")))
        })
        .collect()
}
