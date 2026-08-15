//! Deterministic capability discovery for agents working across the workspace.
//!
//! [`workspace_capabilities`](crate::capability::CapabilityCatalogue) is intentionally a
//! catalogue rather than a vague "the platform supports biology" claim. The discovery contract
//! makes that catalogue useful at runtime: an agent can search by intent, domain, group, or exact
//! tool name and receive ranked, bounded matches with the tools that can actually be called.
//!
//! The catalogue does not infer scientific meaning, access permissions, or readiness. It only
//! ranks explicit labels and preserves the full group record beside the result. Tool schemas are
//! attached by the MCP adapter, which owns the authoritative transport definitions.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Reverse;
use std::collections::BTreeSet;
use thiserror::Error;

/// Wire version for capability discovery requests and responses.
pub const CAPABILITY_SCHEMA_VERSION: &str = "bioprism-devplat-capability/0.1";
const MAX_GROUPS: usize = 512;
const MAX_ITEMS: usize = 500;
const DEFAULT_MAX_ITEMS: usize = 50;
const MAX_FILTER_BYTES: usize = 512;

fn default_max_items() -> usize {
    DEFAULT_MAX_ITEMS
}

fn default_status() -> String {
    "available".into()
}

fn validate_filter(field: &'static str, value: &Option<String>) -> Result<(), CapabilityError> {
    if let Some(value) = value {
        if value.trim().is_empty() {
            return Err(CapabilityError::EmptyFilter { field });
        }
        if value.len() > MAX_FILTER_BYTES {
            return Err(CapabilityError::FilterTooLong {
                field,
                bytes: value.len(),
                maximum: MAX_FILTER_BYTES,
            });
        }
        if value
            .chars()
            .any(|character| character == '\0' || character == '\n' || character == '\r')
        {
            return Err(CapabilityError::ControlCharacter { field });
        }
    }
    Ok(())
}

fn tokens(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn normalized(value: &str) -> String {
    value.to_ascii_lowercase()
}

/// One explicit cross-domain capability group from the workspace catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityGroup {
    pub id: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub crates: Vec<String>,
    #[serde(default)]
    pub mcp_tools: Vec<String>,
    #[serde(default)]
    pub cli_entrypoints: Vec<String>,
    #[serde(default)]
    pub python_artifacts: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,
}

/// Bounded discovery filters. All filters are conjunctive; query terms are additive within the
/// textual search and exact tool/domain/group filters narrow the candidate set first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityQuery {
    /// Free-text intent such as `evidence oncology`, `trace`, or `release provenance`.
    #[serde(default)]
    pub query: Option<String>,
    /// Exact or prefix group identifier, for example `biological_domains`.
    #[serde(default)]
    pub group_id: Option<String>,
    /// Case-insensitive domain label filter.
    #[serde(default)]
    pub domain: Option<String>,
    /// Exact, prefix, or substring MCP tool filter.
    #[serde(default)]
    pub tool: Option<String>,
    /// Maximum ranked groups returned.
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    /// The MCP adapter may attach full authoritative input schemas for matched tools.
    #[serde(default)]
    pub include_tools: bool,
}

impl Default for CapabilityQuery {
    fn default() -> Self {
        Self {
            query: None,
            group_id: None,
            domain: None,
            tool: None,
            max_items: DEFAULT_MAX_ITEMS,
            include_tools: false,
        }
    }
}

impl CapabilityQuery {
    pub fn validate(&self) -> Result<(), CapabilityError> {
        validate_filter("query", &self.query)?;
        validate_filter("group_id", &self.group_id)?;
        validate_filter("domain", &self.domain)?;
        validate_filter("tool", &self.tool)?;
        if !(1..=MAX_ITEMS).contains(&self.max_items) {
            return Err(CapabilityError::InvalidLimit {
                field: "max_items",
                value: self.max_items,
            });
        }
        Ok(())
    }
}

/// One ranked discovery match. The complete group remains beside the score so ranking never
/// silently drops domain, crate, CLI, SDK, or transport context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityMatch {
    pub group: CapabilityGroup,
    pub score: u32,
    pub matched_fields: Vec<String>,
    pub matched_tools: Vec<String>,
}

/// Search response bound to the exact catalogue digest used for ranking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitySearch {
    pub schema_version: String,
    pub catalog_digest: String,
    pub total_groups: usize,
    pub query: CapabilityQuery,
    pub result_count: usize,
    pub matches: Vec<CapabilityMatch>,
}

/// Validated, digest-bound capability catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityCatalogue {
    groups: Vec<CapabilityGroup>,
    digest: ContentHash,
}

impl CapabilityCatalogue {
    /// Parse the transport-neutral array returned by the MCP workspace catalog.
    pub fn from_value(value: &Value) -> Result<Self, CapabilityError> {
        let groups: Vec<CapabilityGroup> = serde_json::from_value(value.clone())
            .map_err(|error| CapabilityError::InvalidCatalogue(error.to_string()))?;
        if groups.is_empty() {
            return Err(CapabilityError::NoGroups);
        }
        if groups.len() > MAX_GROUPS {
            return Err(CapabilityError::TooManyGroups {
                count: groups.len(),
                maximum: MAX_GROUPS,
            });
        }
        let mut ids = BTreeSet::new();
        for group in &groups {
            if group.id.trim().is_empty() {
                return Err(CapabilityError::EmptyGroupId);
            }
            if !ids.insert(group.id.clone()) {
                return Err(CapabilityError::DuplicateGroup {
                    id: group.id.clone(),
                });
            }
            if group.mcp_tools.iter().any(|tool| tool.trim().is_empty()) {
                return Err(CapabilityError::EmptyTool {
                    group: group.id.clone(),
                });
            }
        }
        let encoded = serde_json::to_value(&groups)
            .map_err(|error| CapabilityError::Canonicalisation(error.to_string()))?;
        let digest = ContentHash::of_value(&encoded)
            .map_err(|error| CapabilityError::Canonicalisation(error.to_string()))?;
        Ok(Self { groups, digest })
    }

    pub fn groups(&self) -> &[CapabilityGroup] {
        &self.groups
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    /// Rank groups deterministically. Scores are routing evidence, not scientific or readiness
    /// scores; a caller must inspect the returned labels and tool schemas before acting.
    pub fn search(&self, query: &CapabilityQuery) -> Result<CapabilitySearch, CapabilityError> {
        query.validate()?;
        let query_tokens = query.query.as_deref().map(tokens).unwrap_or_default();
        let group_filter = query.group_id.as_deref().map(normalized);
        let domain_filter = query.domain.as_deref().map(normalized);
        let tool_filter = query.tool.as_deref().map(normalized);
        let mut matches = Vec::new();

        for group in &self.groups {
            let group_id = normalized(&group.id);
            if let Some(filter) = &group_filter {
                if group_id != *filter && !group_id.starts_with(filter) {
                    continue;
                }
            }
            if let Some(filter) = &domain_filter {
                if !group
                    .domains
                    .iter()
                    .any(|domain| normalized(domain).contains(filter))
                {
                    continue;
                }
            }

            let filtered_tools = group
                .mcp_tools
                .iter()
                .filter(|tool| {
                    tool_filter.as_ref().is_none_or(|filter| {
                        let candidate = normalized(tool);
                        candidate == *filter
                            || candidate.starts_with(filter)
                            || candidate.contains(filter)
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            if tool_filter.is_some() && filtered_tools.is_empty() {
                continue;
            }

            let mut score = 0;
            let mut matched_fields = BTreeSet::new();
            if group_filter.is_some() {
                score += if group_id == group_filter.as_deref().unwrap_or_default() {
                    1_000
                } else {
                    700
                };
                matched_fields.insert("group_id".to_string());
            }
            if domain_filter.is_some() {
                score += 500;
                matched_fields.insert("domains".to_string());
            }
            if tool_filter.is_some() {
                score += filtered_tools
                    .iter()
                    .map(|tool| {
                        if normalized(tool) == tool_filter.as_deref().unwrap_or_default() {
                            900
                        } else {
                            600
                        }
                    })
                    .max()
                    .unwrap_or(0);
                matched_fields.insert("mcp_tools".to_string());
            }

            let searchable = [
                ("group_id", group.id.as_str()),
                ("domains", &group.domains.join(" ")),
                ("crates", &group.crates.join(" ")),
                ("mcp_tools", &group.mcp_tools.join(" ")),
                ("cli_entrypoints", &group.cli_entrypoints.join(" ")),
                ("python_artifacts", &group.python_artifacts.join(" ")),
            ];
            let mut all_query_tokens_match = true;
            for query_token in &query_tokens {
                let mut fields_for_token = Vec::new();
                for (field, value) in &searchable {
                    let field_tokens = tokens(value);
                    if field_tokens.iter().any(|candidate| {
                        candidate == query_token || candidate.starts_with(query_token)
                    }) {
                        fields_for_token.push(*field);
                    }
                }
                if fields_for_token.is_empty() {
                    all_query_tokens_match = false;
                    break;
                }
                for field in fields_for_token {
                    matched_fields.insert(field.to_string());
                    score += if field == "mcp_tools" { 140 } else { 100 };
                }
            }
            if !query_tokens.is_empty() && !all_query_tokens_match {
                continue;
            }

            let matched_tools = if tool_filter.is_some() {
                filtered_tools
            } else if !query_tokens.is_empty() {
                let query_tools = group
                    .mcp_tools
                    .iter()
                    .filter(|tool| {
                        let tool_tokens = tokens(tool);
                        query_tokens.iter().any(|query_token| {
                            tool_tokens.iter().any(|candidate| {
                                candidate == query_token || candidate.starts_with(query_token)
                            })
                        })
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if query_tools.is_empty() {
                    group.mcp_tools.clone()
                } else {
                    query_tools
                }
            } else {
                group.mcp_tools.clone()
            };
            matches.push(CapabilityMatch {
                group: group.clone(),
                score,
                matched_fields: matched_fields.into_iter().collect(),
                matched_tools,
            });
        }

        matches.sort_by_key(|matched| (Reverse(matched.score), matched.group.id.clone()));
        matches.truncate(query.max_items);
        Ok(CapabilitySearch {
            schema_version: CAPABILITY_SCHEMA_VERSION.into(),
            catalog_digest: self.digest.to_string(),
            total_groups: self.groups.len(),
            query: query.clone(),
            result_count: matches.len(),
            matches,
        })
    }
}

/// Fail-closed catalogue and query errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CapabilityError {
    #[error("capability catalogue is empty")]
    NoGroups,
    #[error("capability catalogue contains too many groups: {count}; maximum is {maximum}")]
    TooManyGroups { count: usize, maximum: usize },
    #[error("capability group id must be non-empty")]
    EmptyGroupId,
    #[error("duplicate capability group `{id}`")]
    DuplicateGroup { id: String },
    #[error("capability group `{group}` contains an empty MCP tool name")]
    EmptyTool { group: String },
    #[error("{field} filter must be non-empty when supplied")]
    EmptyFilter { field: &'static str },
    #[error("{field} filter contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("{field} filter is {bytes} bytes; maximum is {maximum}")]
    FilterTooLong {
        field: &'static str,
        bytes: usize,
        maximum: usize,
    },
    #[error("max_items must be between 1 and the safety ceiling, got {value}")]
    InvalidLimit { field: &'static str, value: usize },
    #[error("invalid capability catalogue: {0}")]
    InvalidCatalogue(String),
    #[error("cannot canonicalise capability catalogue: {0}")]
    Canonicalisation(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalogue() -> CapabilityCatalogue {
        CapabilityCatalogue::from_value(&json!([
            {
                "id": "biological_domains",
                "domains": ["oncology", "modalities"],
                "crates": ["bioprism-onco"],
                "mcp_tools": ["onco_response_assess", "modality_catalog"],
                "cli_entrypoints": [],
                "status": "available"
            },
            {
                "id": "release_and_reproduction",
                "domains": ["release", "provenance"],
                "crates": ["bioprism-bundle"],
                "mcp_tools": ["bundle_verify", "release_audit"],
                "cli_entrypoints": [],
                "status": "available"
            },
            {
                "id": "trace_and_telemetry",
                "domains": ["observability", "traces"],
                "crates": ["bioprism-trace"],
                "mcp_tools": ["trace_analyze", "telemetry_project"],
                "cli_entrypoints": [],
                "status": "available"
            }
        ]))
        .unwrap()
    }

    #[test]
    fn search_is_ranked_bounded_and_keeps_routing_tools() {
        let result = catalogue()
            .search(&CapabilityQuery {
                query: Some("oncology".into()),
                max_items: 1,
                ..CapabilityQuery::default()
            })
            .unwrap();
        assert_eq!(result.result_count, 1);
        assert_eq!(result.matches[0].group.id, "biological_domains");
        assert!(result.matches[0]
            .matched_tools
            .contains(&"onco_response_assess".into()));
        assert_eq!(result.catalog_digest.len(), 64);
    }

    #[test]
    fn exact_tool_and_domain_filters_are_conjunctive() {
        let result = catalogue()
            .search(&CapabilityQuery {
                domain: Some("release".into()),
                tool: Some("bundle_verify".into()),
                ..CapabilityQuery::default()
            })
            .unwrap();
        assert_eq!(result.result_count, 1);
        assert_eq!(result.matches[0].matched_tools, vec!["bundle_verify"]);
        let empty = catalogue()
            .search(&CapabilityQuery {
                domain: Some("release".into()),
                tool: Some("trace_analyze".into()),
                ..CapabilityQuery::default()
            })
            .unwrap();
        assert!(empty.matches.is_empty());
    }

    #[test]
    fn malformed_limits_and_filters_refuse() {
        assert!(matches!(
            catalogue().search(&CapabilityQuery {
                max_items: 0,
                ..CapabilityQuery::default()
            }),
            Err(CapabilityError::InvalidLimit { .. })
        ));
        assert!(matches!(
            catalogue().search(&CapabilityQuery {
                query: Some("\n".into()),
                ..CapabilityQuery::default()
            }),
            Err(CapabilityError::EmptyFilter { .. })
                | Err(CapabilityError::ControlCharacter { .. })
        ));
    }

    #[test]
    fn duplicate_groups_and_empty_tools_refuse_catalogue() {
        let value = json!([
            {"id": "one", "mcp_tools": ["tool"]},
            {"id": "one", "mcp_tools": ["other"]}
        ]);
        assert!(matches!(
            CapabilityCatalogue::from_value(&value),
            Err(CapabilityError::DuplicateGroup { .. })
        ));
        let value = json!([{"id": "one", "mcp_tools": [""]}]);
        assert!(matches!(
            CapabilityCatalogue::from_value(&value),
            Err(CapabilityError::EmptyTool { .. })
        ));
    }
}
