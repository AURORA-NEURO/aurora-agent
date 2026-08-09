//! Query IR: the typed decision contract.
//!
//! Blueprint 43.02 defines a query as `q = ⟨Y, A, t, ω, ℓ, B, ε, R⟩` — targets, permitted
//! actions, time cut, role/policy, decision loss, budget, tolerated distortion and requested
//! outputs. The `fiber-query/0.1` wire schema carries only targets, protected tags, decision
//! time, budgets, role, policy and distortion tolerance.
//!
//! The permitted-action set `A` and the decision loss `ℓ` are **absent from the wire format**.
//! That matters: decision-equivalence quotienting (43.10) and rate-distortion optimisation
//! (43.12) are both defined relative to `A` and `ℓ`, so neither can be implemented against v0.1
//! without extending the schema. [`Query::missing_contract_fields`] reports the gap rather than
//! letting a later pass quietly substitute a default.
//!
//! Of the role/policy pair `ω`, only [`Query::policy`] is read — by [`crate::policy`], as the set
//! of obligations the caller accepts. [`Query::role`] is still parsed and discarded: 43.33 binds
//! role and purpose to the query at step 1, `bioprism-scope` registers `role` and `visibility` as
//! policy-class dimensions, and no pass in this crate consults either. That is the same defect the
//! §40 audit found in `policy`, one field over, and it is recorded here rather than fixed silently.

use crate::error::FiberError;
use bioprism_ids::{QueryId, VariableName};
use bioprism_scope::Timestamp;
use serde_json::Value;
use std::collections::BTreeSet;

pub const QUERY_SCHEMA_VERSION: &str = "fiber-query/0.1";

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
        if schema_version != QUERY_SCHEMA_VERSION {
            return Err(FiberError::UnsupportedQuerySchema {
                expected: QUERY_SCHEMA_VERSION,
                actual: schema_version.to_string(),
            });
        }

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

    /// Components of the formal decision contract that `fiber-query/0.1` cannot express.
    ///
    /// Any pass that needs one of these must refuse to run rather than assume a default.
    pub fn missing_contract_fields(&self) -> Vec<&'static str> {
        let mut missing = vec!["permitted_actions", "decision_loss"];
        if self.distortion_tolerance.is_none() {
            missing.push("distortion_tolerance");
        }
        missing
    }
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
