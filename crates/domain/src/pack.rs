//! The domain-pack document: one decision question, declared whole.
//!
//! A pack binds together the three things a non-reference domain has to supply before the
//! pipeline can serve it honestly: the scope dimensions its worlds' facts are scoped by, the
//! tags its queries should place in protected closure, and the rule oracle that judges the
//! compiled value map. Loading a pack validates all three; nothing about a pack is consulted
//! lazily, so a malformed pack fails at the boundary rather than mid-compile.
//!
//! A pack deliberately does not carry worlds or queries. Worlds are evidence, queries are
//! decisions, and both bind to the pack only at compile time, through
//! [`bioprism_fiber::compile_with_oracle`] — the certificate then names the pack's oracle kind,
//! which is the only coupling a verifier needs.

use crate::rules::RuleOracle;
use crate::DomainError;
use bioprism_scope::DimensionRegistry;
use serde_json::{Map, Value};

/// The wire version of the domain-pack document.
pub const DOMAIN_SCHEMA_VERSION: &str = "bioprism-domain/0.1";

/// A parsed, validated domain pack.
#[derive(Debug, Clone)]
pub struct DomainPack {
    name: String,
    description: String,
    goal: Option<String>,
    protected_tags: Vec<String>,
    scope_dimensions: Option<Value>,
    oracle: RuleOracle,
}

impl DomainPack {
    /// Parses the strict wire form.
    ///
    /// Declared keys: `schema_version`, `name`, `description`, `goal` (optional),
    /// `protected_tags` (optional), `scope_dimensions` (optional, a
    /// `bioprism-scope-dimensions/0.1` document validated on load), `oracle`.
    pub fn from_json(document: &Value) -> Result<DomainPack, DomainError> {
        let map = document
            .as_object()
            .ok_or_else(|| pack_error("domain pack is not an object"))?;

        match map.get("schema_version").and_then(Value::as_str) {
            Some(DOMAIN_SCHEMA_VERSION) => {}
            Some(other) => {
                return Err(pack_error(&format!(
                    "unsupported schema {other:?}; expected {DOMAIN_SCHEMA_VERSION:?}"
                )))
            }
            None => return Err(pack_error("domain pack declares no schema_version")),
        }

        const DECLARED: &[&str] = &[
            "schema_version",
            "name",
            "description",
            "goal",
            "protected_tags",
            "scope_dimensions",
            "oracle",
        ];
        if let Some(unknown) = map.keys().find(|key| !DECLARED.contains(&key.as_str())) {
            return Err(pack_error(&format!(
                "undeclared key {unknown:?} in domain pack"
            )));
        }

        let name = required_string(map, "name")?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(pack_error(&format!(
                "pack name {name:?} must be a non-empty lowercase-ascii slug (a-z, 0-9, '-')"
            )));
        }
        let description = required_string(map, "description")?;

        let goal = match map.get("goal") {
            None => None,
            Some(value) => Some(
                value
                    .as_str()
                    .map(str::to_string)
                    .ok_or_else(|| pack_error("\"goal\" is not a string"))?,
            ),
        };

        let protected_tags = match map.get("protected_tags") {
            None => Vec::new(),
            Some(list) => list
                .as_array()
                .ok_or_else(|| pack_error("\"protected_tags\" is not an array"))?
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| pack_error("\"protected_tags\" carries a non-string entry"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };

        let scope_dimensions = map.get("scope_dimensions").cloned();
        if let Some(dimensions) = &scope_dimensions {
            DimensionRegistry::from_json(dimensions).map_err(DomainError::Dimensions)?;
        }

        let oracle = RuleOracle::from_json(
            map.get("oracle")
                .ok_or_else(|| pack_error("domain pack declares no \"oracle\""))?,
        )?;

        Ok(DomainPack {
            name,
            description,
            goal,
            protected_tags,
            scope_dimensions,
            oracle,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    /// The goal a query in this domain should declare. Advisory: the pack cannot inject it into
    /// a query, because mutating a query would change the bytes its certificate binds by hash.
    pub fn goal(&self) -> Option<&str> {
        self.goal.as_deref()
    }

    /// The tags a query in this domain should place in protected closure. Advisory for the same
    /// reason as [`DomainPack::goal`]: the query stays the sole author of its own contract.
    pub fn protected_tags(&self) -> &[String] {
        &self.protected_tags
    }

    /// The oracle this pack declares, ready for [`bioprism_fiber::compile_with_oracle`].
    pub fn oracle(&self) -> &RuleOracle {
        &self.oracle
    }

    /// The default dimension table extended by this pack's declared dimensions.
    ///
    /// Validated at parse time, so this cannot fail after [`DomainPack::from_json`] returned.
    pub fn dimension_registry(&self) -> DimensionRegistry {
        match &self.scope_dimensions {
            None => DimensionRegistry::default(),
            Some(dimensions) => DimensionRegistry::from_json(dimensions)
                .expect("dimension document was validated when the pack was parsed"),
        }
    }
}

fn required_string(map: &Map<String, Value>, field: &str) -> Result<String, DomainError> {
    map.get(field)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| pack_error(&format!("domain pack needs a string {field:?}")))
}

fn pack_error(message: &str) -> DomainError {
    DomainError::Pack(message.to_string())
}
