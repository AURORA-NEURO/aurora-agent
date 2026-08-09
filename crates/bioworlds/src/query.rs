//! The query *shape* a slice pairs with its world.
//!
//! Blueprint 43.41 pairs each reference world with a typed decision query. This crate cannot
//! compile one: `bioprism-fiber` is deliberately absent from its dependency set, because a world
//! builder that could run the compiler would be tempted to tune worlds until the compiler looked
//! good on them. So what a slice carries is the query *shape* — target, protected tag vocabulary,
//! decision cut, budgets — serialised in the `fiber-query/0.1` wire format so that a consumer who
//! does link the compiler can feed it unchanged.
//!
//! # What this type can and cannot tell you
//!
//! It fixes which facts are protected and where the temporal cut falls, which is enough to decide
//! *structural* questions: is this variable in the protected closure, is it readable at the cut,
//! does a neighbourhood ball of radius d contain it. It says nothing about verdicts. Nowhere in
//! this crate is a compiled verdict claimed, asserted or implied.

use crate::error::BioWorldError;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// The wire schema a query shape serialises to.
pub const QUERY_SCHEMA_VERSION: &str = "fiber-query/0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryShape {
    pub query_id: String,
    /// The decision variables. Every world in this crate has exactly one.
    pub targets: Vec<String>,
    /// Tags whose facts enter the selection before any relevance step (43.13).
    pub protected_tags: BTreeSet<String>,
    /// The temporal cut. Evidence whose governing event is released after this instant is
    /// unreadable, whatever the relevance step thinks of it (43.09).
    pub decision_time: String,
    pub max_facts: usize,
    pub max_tokens: usize,
    pub role: String,
    pub policy: Vec<String>,
}

impl QueryShape {
    /// The parsed cut, or a typed error naming the offending literal.
    pub fn cut(&self) -> Result<Timestamp, BioWorldError> {
        Timestamp::parse(&self.decision_time).map_err(|source| BioWorldError::Timestamp {
            subject: format!("query {}", self.query_id),
            value: self.decision_time.clone(),
            message: source.to_string(),
        })
    }

    /// The single target, when there is exactly one.
    pub fn target(&self) -> Option<&str> {
        match self.targets.as_slice() {
            [only] => Some(only.as_str()),
            _ => None,
        }
    }

    pub fn protects(&self, tag: &str) -> bool {
        self.protected_tags.contains(tag)
    }

    /// The `fiber-query/0.1` document, for a consumer that owns a compiler.
    pub fn to_document(&self) -> Value {
        json!({
            "schema_version": QUERY_SCHEMA_VERSION,
            "query_id": self.query_id,
            "targets": self.targets,
            "protected_tags": self.protected_tags,
            "decision_time": self.decision_time,
            "budgets": { "max_facts": self.max_facts, "max_tokens": self.max_tokens },
            "role": self.role,
            "policy": self.policy,
            "distortion_tolerance": 0.0
        })
    }

    /// The protected vocabulary split into whole tokens.
    ///
    /// Tag camouflage (43.39) works because closure matches *whole tags* while a lexical retriever
    /// tokenises: `identity_summary` is correctly outside the closure and still scores against a
    /// query mentioning `identity`. Measuring camouflage therefore needs the token set, not the
    /// tag set.
    pub fn protected_tokens(&self) -> BTreeSet<String> {
        self.protected_tags
            .iter()
            .flat_map(|tag| tag.split('_').map(str::to_string))
            .collect()
    }
}

/// Builds a tag set without forcing every caller to write the same `into`/`collect` chain.
pub fn tag_set(tags: &[&str]) -> BTreeSet<String> {
    tags.iter().map(|tag| (*tag).to_string()).collect()
}
