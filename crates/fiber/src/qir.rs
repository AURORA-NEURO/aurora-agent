//! Query IR: the typed decision contract.
//!
//! Blueprint 43.02 defines a query as `q = ⟨Y, A, t, ω, ℓ, B, ε, R⟩` — targets, permitted
//! actions, time cut, role/policy, decision loss, budget, tolerated distortion and requested
//! outputs. The `fiber-query/0.2` wire schema carries only targets, protected tags, decision
//! time, budgets, role, policy, distortion tolerance and goal.
//!
//! The permitted-action set `A` and the decision loss `ℓ` are **absent from the wire format**.
//! That matters: decision-equivalence quotienting (43.10) and rate-distortion optimisation
//! (43.12) are both defined relative to `A` and `ℓ`, so neither can be implemented against 0.2
//! without extending the schema. [`Query::missing_contract_fields`] reports the gap rather than
//! letting a later pass quietly substitute a default.
//!
//! Of the role/policy pair `ω`, only [`Query::policy`] is read — by [`crate::policy`], as the set
//! of obligations the caller accepts. [`Query::role`] is still parsed and discarded: 43.33 binds
//! role and purpose to the query at step 1, `bioprism-scope` registers `role` and `visibility` as
//! policy-class dimensions, and no pass in this crate consults either. That is the same defect the
//! §40 audit found in `policy`, one field over, and it is recorded here rather than fixed silently.
//!
//! # 0.2: an undeclared key is refused
//!
//! Through 0.1 this parser read the keys it knew and dropped the rest. That is cheap for a format
//! nobody hashes, and wrong for this one, because the *whole query document* is hashed into
//! `source_hashes.query_sha256` and thence into the certificate's own digest. An undeclared key
//! was therefore ignored semantically and honoured cryptographically: it could move a compiled
//! answer's identity without being able to move the answer. [`FiberError::UnknownQueryFields`]
//! closes both halves, and [`QUERY_FIELD_PATHS`] is the accepted set the refusal quotes.
//!
//! Note what did *not* change: no field was added, removed or retyped. The only thing 0.2 moves is
//! the reader's declared handling of a key nobody declared — `bioprism-governance` calls that a
//! `CompatibilityMode` change, classifies it `Breaking` on its own terms, and reports zero
//! digest-affecting changes. Both halves of that are load-bearing here and both are asserted in
//! `tests/unknown_query_keys.rs`.
//!
//! # Why a 0.1 label is still read
//!
//! [`ACCEPTED_QUERY_SCHEMA_VERSIONS`] holds two entries, which looks like a loophole and is the
//! opposite. The published `schemas/fiber-v0.1/query.schema.json` has declared
//! `additionalProperties: false` since 0.1: a document carrying an undeclared key was *never* a
//! valid 0.1 query, the parser simply was not enforcing the schema it published. So no document
//! that was ever valid is invalidated by this change, and re-labelling every existing query to 0.2
//! would move `query_sha256` — and therefore the certificate digest — of documents whose content
//! did not change, to record a change in the reader. The reference pair in `fixtures/fiber-v0.1/`
//! is precisely such a document: it is the frozen artifact three implementations agree on, and a
//! 0.2 reader has to be able to replay it or the parity anchor is gone.
//!
//! # Why `fiber-world/0.1` is a separate change
//!
//! It has the identical hole, and worse: [`bioprism_world::World::from_json`] reads named keys and
//! retains the raw document *deliberately*, because certificate hashes are taken over the original
//! bytes. So a world with an undeclared key also compiles to a different certificate digest with
//! identical content. It is not fixed here, for three reasons.
//!
//! First, it is a different named format, so it is a second lineage: `SchemaId::successor_of`
//! refuses to compare `fiber-query` with `fiber-world`, and one classification cannot cover both.
//! Second, `World::validate_reference_compat` is documented as "deliberately no stricter" than the
//! CPython `validate_world`, and its dependent set — `worldgen`, `bioworlds`, `oncoworlds`,
//! `packs`, `store`, `backends` and every world fixture in the tree — is an order of magnitude
//! larger than the query's. Third, and decisively, a query is flat while a world is not: `scope`
//! is an open extension point that `bioprism-scope` registers dimensions into, so "undeclared key"
//! inside a fact's scope needs a design decision this change has no basis to make. Two changes,
//! and this is the first.

use crate::error::FiberError;
use bioprism_ids::{QueryId, VariableName};
use bioprism_scope::Timestamp;
use serde_json::{Map, Value};
use std::collections::BTreeSet;

/// The version a query this compiler emits or prefers declares.
pub const QUERY_SCHEMA_VERSION: &str = "fiber-query/0.2";

/// Every version label this parser reads, newest last. See the module docs for why 0.1 is here.
pub const ACCEPTED_QUERY_SCHEMA_VERSIONS: &[&str] = &["fiber-query/0.1", QUERY_SCHEMA_VERSION];

/// Every key `fiber-query/0.2` declares, as dotted paths, sorted.
///
/// Sorted because this array is quoted verbatim in [`FiberError::UnknownQueryFields`], and an
/// error message whose contents depend on declaration order is a diff nobody can review. Dotted
/// because `budgets` is the format's one nested object and the published schema closes it too, so
/// `budgets.max_items` has to be refusable by a name that says where it was found.
///
/// `tests/unknown_query_keys.rs` holds this against `schemas/fiber-v0.1/query.schema.json` as an
/// equality, the way `bioprism-governance`'s `known.rs` does for the certificate. A key the parser
/// accepts and the published schema omits is the same defect in the other direction.
pub const QUERY_FIELD_PATHS: &[&str] = &[
    "budgets",
    "budgets.max_facts",
    "budgets.max_tokens",
    "decision_time",
    "distortion_tolerance",
    "goal",
    "policy",
    "protected_tags",
    "query_id",
    "role",
    "schema_version",
    "targets",
];

/// The goal string the CPython reference hard-codes into every Decision Section.
///
/// Hard-coding a radiogenomic goal inside a general compiler is a defect in the reference. A
/// query may supply its own `goal`; when it does not, this value is used so that sections remain
/// byte-comparable with the reference.
pub const REFERENCE_GOAL: &str =
    "Determine whether the proposed radiogenomic split supports a valid external-generalization analysis.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budgets {
    pub max_facts: usize,
    pub max_tokens: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Query {
    pub query_id: QueryId,
    pub targets: Vec<VariableName>,
    pub protected_tags: BTreeSet<String>,
    pub decision_time: Timestamp,
    /// The decision time exactly as written, echoed into the section verbatim.
    pub decision_time_raw: String,
    pub budgets: Budgets,
    pub role: Option<String>,
    pub policy: Vec<String>,
    pub distortion_tolerance: Option<f64>,
    pub goal: Option<String>,
    raw: Value,
}

impl Query {
    pub fn from_json(raw: Value) -> Result<Self, FiberError> {
        let map = raw.as_object().ok_or(FiberError::QueryNotAnObject)?;

        let schema_version = map
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !ACCEPTED_QUERY_SCHEMA_VERSIONS.contains(&schema_version) {
            return Err(FiberError::UnsupportedQuerySchema {
                expected: ACCEPTED_QUERY_SCHEMA_VERSIONS,
                actual: schema_version.to_string(),
            });
        }

        reject_undeclared_fields(map)?;

        let query_id_text = map
            .get("query_id")
            .and_then(Value::as_str)
            .ok_or(FiberError::MissingQueryField("query_id"))?;
        let query_id = QueryId::parse(query_id_text)
            .map_err(|e| FiberError::InvalidIdentifier(e.to_string()))?;

        let targets_raw = map
            .get("targets")
            .and_then(Value::as_array)
            .ok_or(FiberError::MissingQueryField("targets"))?;
        let targets = targets_raw
            .iter()
            .map(|item| {
                item.as_str()
                    .ok_or(FiberError::WrongQueryFieldType {
                        field: "targets",
                        expected: "array of strings",
                    })
                    .and_then(|name| {
                        VariableName::parse(name)
                            .map_err(|e| FiberError::InvalidIdentifier(e.to_string()))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let decision_time_raw = map
            .get("decision_time")
            .and_then(Value::as_str)
            .ok_or(FiberError::MissingQueryField("decision_time"))?
            .to_string();
        let decision_time = Timestamp::parse(&decision_time_raw)
            .map_err(|e| FiberError::InvalidDecisionTime(e.to_string()))?;

        let budgets_map = map
            .get("budgets")
            .and_then(Value::as_object)
            .ok_or(FiberError::MissingQueryField("budgets"))?;
        let max_facts = budgets_map
            .get("max_facts")
            .and_then(Value::as_u64)
            .ok_or(FiberError::MissingQueryField("budgets.max_facts"))?
            as usize;

        Ok(Query {
            query_id,
            targets,
            protected_tags: string_set(map.get("protected_tags")),
            decision_time,
            decision_time_raw,
            budgets: Budgets {
                max_facts,
                max_tokens: budgets_map
                    .get("max_tokens")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize),
            },
            role: map.get("role").and_then(Value::as_str).map(str::to_string),
            policy: string_set(map.get("policy")).into_iter().collect(),
            distortion_tolerance: map.get("distortion_tolerance").and_then(Value::as_f64),
            goal: map.get("goal").and_then(Value::as_str).map(str::to_string),
            raw,
        })
    }

    pub fn raw(&self) -> &Value {
        &self.raw
    }

    pub fn goal_text(&self) -> &str {
        self.goal.as_deref().unwrap_or(REFERENCE_GOAL)
    }

    /// Components of the formal decision contract that `fiber-query/0.2` still cannot express.
    ///
    /// Any pass that needs one of these must refuse to run rather than assume a default. 0.2
    /// refused the *undeclared* key without adding the fields, so a caller who writes
    /// `decision_loss` now gets an error naming it instead of silence — but the gap
    /// `bioprism-epistemic::gap` specifies is still a gap, and this list still names it.
    pub fn missing_contract_fields(&self) -> Vec<&'static str> {
        let mut missing = vec!["permitted_actions", "decision_loss"];
        if self.distortion_tolerance.is_none() {
            missing.push("distortion_tolerance");
        }
        missing
    }
}

/// Refuses a query carrying any key outside [`QUERY_FIELD_PATHS`].
///
/// Runs immediately after the version check and before any field is read, so a caller is told
/// their document is not a query of this format before being told which of its fields is missing.
/// Reporting `missing required query field "targets"` about a document that also carries a
/// misspelled `target` would send them after the wrong problem.
///
/// `budgets` is descended into and nothing else is, because it is the only nested object the
/// format declares. `policy`, `protected_tags` and `targets` are arrays of strings with no
/// interior to declare, and a value that is not an object cannot hide a key.
fn reject_undeclared_fields(map: &Map<String, Value>) -> Result<(), FiberError> {
    let mut undeclared: Vec<String> = map
        .keys()
        .filter(|key| !QUERY_FIELD_PATHS.contains(&key.as_str()))
        .cloned()
        .collect();

    if let Some(Value::Object(budgets)) = map.get("budgets") {
        undeclared.extend(
            budgets
                .keys()
                .map(|key| format!("budgets.{key}"))
                .filter(|path| !QUERY_FIELD_PATHS.contains(&path.as_str())),
        );
    }

    if undeclared.is_empty() {
        return Ok(());
    }
    undeclared.sort();
    Err(FiberError::UnknownQueryFields {
        fields: undeclared,
        accepted: QUERY_FIELD_PATHS,
    })
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}
