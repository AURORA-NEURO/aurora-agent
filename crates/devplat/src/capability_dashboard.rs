//! Digest-bound capability coverage for agents choosing among workspace domains.
//!
//! Discovery answers which groups match a query and the capability audit checks catalogue/schema
//! parity. This module adds the operator-facing projection between them: every selected group is
//! classified as callable, partial, or declared-only, with explicit transport, CLI, Python, and
//! crate-ownership gaps. It never turns a catalogue row into permission or scientific readiness.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::capability::CapabilityCatalogue;

pub const CAPABILITY_DASHBOARD_SCHEMA: &str = "bioprism-devplat-capability-dashboard/0.1";
pub const MAX_DASHBOARD_GROUPS: usize = 512;
pub const DEFAULT_DASHBOARD_GROUPS: usize = 128;

fn default_max_groups() -> usize {
    DEFAULT_DASHBOARD_GROUPS
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityDashboardQuery {
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default = "default_max_groups")]
    pub max_groups: usize,
    #[serde(default)]
    pub include_tools: bool,
    #[serde(default = "default_true")]
    pub include_gaps: bool,
}

fn default_true() -> bool {
    true
}

impl Default for CapabilityDashboardQuery {
    fn default() -> Self {
        Self {
            group_id: None,
            domain: None,
            status: None,
            max_groups: DEFAULT_DASHBOARD_GROUPS,
            include_tools: false,
            include_gaps: true,
        }
    }
}

impl CapabilityDashboardQuery {
    pub fn validate(&self) -> Result<(), CapabilityDashboardError> {
        for (field, value) in [
            ("group_id", self.group_id.as_ref()),
            ("domain", self.domain.as_ref()),
            ("status", self.status.as_ref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() {
                    return Err(CapabilityDashboardError::EmptyFilter { field });
                }
                if value.len() > 512 {
                    return Err(CapabilityDashboardError::FilterTooLong { field });
                }
                if value.chars().any(char::is_control) {
                    return Err(CapabilityDashboardError::ControlCharacter { field });
                }
            }
        }
        if !(1..=MAX_DASHBOARD_GROUPS).contains(&self.max_groups) {
            return Err(CapabilityDashboardError::InvalidLimit {
                field: "max_groups",
                value: self.max_groups,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityDashboardGroup {
    pub id: String,
    pub domains: Vec<String>,
    pub status: String,
    pub readiness: String,
    pub surfaces: CapabilityDashboardSurfaces,
    pub tool_count: usize,
    pub callable_tool_count: usize,
    pub schema_backed_tool_count: usize,
    pub missing_transport_schemas: Vec<String>,
    pub invalid_transport_schemas: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gaps: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityDashboardSurfaces {
    pub crates: usize,
    pub mcp_tools: usize,
    pub cli_entrypoints: usize,
    pub python_artifacts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityDashboardAudit {
    pub schema: String,
    pub catalog_digest: String,
    pub dashboard_digest: String,
    pub query: CapabilityDashboardQuery,
    pub total_group_count: usize,
    pub selected_group_count: usize,
    pub available_group_count: usize,
    pub callable_group_count: usize,
    pub partial_group_count: usize,
    pub declared_only_group_count: usize,
    pub selected_tool_memberships: usize,
    pub selected_unique_tools: usize,
    pub schema_backed_unique_tools: usize,
    pub readiness_counts: BTreeMap<String, usize>,
    pub gap_counts: BTreeMap<String, usize>,
    pub groups: Vec<CapabilityDashboardGroup>,
    pub warnings: Vec<String>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
    pub ready: bool,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CapabilityDashboardError {
    #[error("{field} filter must be non-empty when supplied")]
    EmptyFilter { field: &'static str },
    #[error("{field} filter exceeds the 512-byte safety bound")]
    FilterTooLong { field: &'static str },
    #[error("{field} filter contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("{field} must be between 1 and {MAX_DASHBOARD_GROUPS}, got {value}")]
    InvalidLimit { field: &'static str, value: usize },
    #[error("cannot canonicalize capability dashboard: {0}")]
    Canonical(String),
}

/// Build a dashboard from the authoritative catalogue and a name-to-schema-quality map.
///
/// The MCP layer owns schema inspection because it owns `tools/list`; this pure projection only
/// consumes the resulting booleans. A missing map entry means no authoritative transport schema.
pub fn build_dashboard(
    catalogue: &CapabilityCatalogue,
    tool_schema_quality: &BTreeMap<String, bool>,
    query: &CapabilityDashboardQuery,
) -> Result<CapabilityDashboardAudit, CapabilityDashboardError> {
    query.validate()?;
    let group_filter = query
        .group_id
        .as_ref()
        .map(|value| value.to_ascii_lowercase());
    let domain_filter = query
        .domain
        .as_ref()
        .map(|value| value.to_ascii_lowercase());
    let status_filter = query
        .status
        .as_ref()
        .map(|value| value.to_ascii_lowercase());
    let mut groups = Vec::new();
    let mut total_available = 0;
    let mut gap_counts = BTreeMap::new();
    let mut readiness_counts = BTreeMap::new();
    let mut selected_tool_memberships = 0;
    let mut selected_tools = BTreeSet::new();
    let mut schema_backed_tools = BTreeSet::new();
    let mut truncated = false;

    let mut source_groups = catalogue.groups().iter().collect::<Vec<_>>();
    source_groups.sort_by(|left, right| left.id.cmp(&right.id));
    for group in source_groups {
        if group_filter
            .as_ref()
            .is_some_and(|filter| !group.id.to_ascii_lowercase().starts_with(filter))
        {
            continue;
        }
        if domain_filter.as_ref().is_some_and(|filter| {
            !group
                .domains
                .iter()
                .any(|domain| domain.to_ascii_lowercase().contains(filter))
        }) {
            continue;
        }
        if status_filter
            .as_ref()
            .is_some_and(|filter| group.status.to_ascii_lowercase() != *filter)
        {
            continue;
        }
        if groups.len() >= query.max_groups {
            truncated = true;
            break;
        }
        if group.status == "available" {
            total_available += 1;
        }

        let tools = group.mcp_tools.iter().cloned().collect::<BTreeSet<_>>();
        let missing = tools
            .iter()
            .filter(|tool| !tool_schema_quality.contains_key(*tool))
            .cloned()
            .collect::<Vec<_>>();
        let invalid = tools
            .iter()
            .filter(|tool| tool_schema_quality.get(*tool) == Some(&false))
            .cloned()
            .collect::<Vec<_>>();
        let callable_tool_count = tools
            .iter()
            .filter(|tool| tool_schema_quality.get(*tool) == Some(&true))
            .count();
        let schema_backed_tool_count = tools
            .iter()
            .filter(|tool| tool_schema_quality.get(*tool) == Some(&true))
            .count();
        let readiness = if tools.is_empty() {
            "declared_only"
        } else if missing.is_empty() && invalid.is_empty() {
            "callable"
        } else {
            "partial"
        };
        *readiness_counts.entry(readiness.into()).or_insert(0) += 1;
        selected_tool_memberships += group.mcp_tools.len();
        selected_tools.extend(tools.iter().cloned());
        schema_backed_tools.extend(
            tools
                .iter()
                .filter(|tool| tool_schema_quality.get(*tool) == Some(&true))
                .cloned(),
        );

        let mut gaps = Vec::new();
        if group.crates.is_empty() {
            gaps.push("no_crate_ownership".to_string());
        }
        if group.cli_entrypoints.is_empty() {
            gaps.push("no_cli_entrypoints".to_string());
        }
        if group.python_artifacts.is_empty() {
            gaps.push("no_python_artifact".to_string());
        }
        if tools.is_empty() {
            gaps.push("no_mcp_tools".to_string());
        }
        if !missing.is_empty() {
            gaps.push("missing_transport_schema".to_string());
        }
        if !invalid.is_empty() {
            gaps.push("invalid_transport_schema".to_string());
        }
        for gap in &gaps {
            *gap_counts.entry(gap.clone()).or_insert(0) += 1;
        }
        groups.push(CapabilityDashboardGroup {
            id: group.id.clone(),
            domains: group.domains.clone(),
            status: group.status.clone(),
            readiness: readiness.into(),
            surfaces: CapabilityDashboardSurfaces {
                crates: group.crates.len(),
                mcp_tools: group.mcp_tools.len(),
                cli_entrypoints: group.cli_entrypoints.len(),
                python_artifacts: group.python_artifacts.len(),
            },
            tool_count: tools.len(),
            callable_tool_count,
            schema_backed_tool_count,
            missing_transport_schemas: missing,
            invalid_transport_schemas: invalid,
            tools: query.include_tools.then(|| tools.into_iter().collect()),
            gaps: query.include_gaps.then_some(gaps),
        });
    }

    let mut warnings: Vec<String> = Vec::new();
    if groups.is_empty() {
        warnings.push("no capability groups matched the dashboard query".into());
    }
    if truncated {
        warnings.push(
            "dashboard group output is bounded; inspect max_groups before claiming completeness"
                .into(),
        );
    }
    let ready = !groups.is_empty()
        && groups.iter().all(|group| group.readiness == "callable")
        && warnings.iter().all(|warning| !warning.contains("bounded"));
    let dashboard_value = serde_json::to_value((
        catalogue.digest().to_string(),
        query,
        &groups,
        &readiness_counts,
        &gap_counts,
    ))
    .map_err(|error| CapabilityDashboardError::Canonical(error.to_string()))?;
    let dashboard_digest = ContentHash::of_value(&dashboard_value)
        .map_err(|error| CapabilityDashboardError::Canonical(error.to_string()))?
        .to_string();
    Ok(CapabilityDashboardAudit {
        schema: CAPABILITY_DASHBOARD_SCHEMA.into(),
        catalog_digest: catalogue.digest().to_string(),
        dashboard_digest,
        query: query.clone(),
        total_group_count: catalogue.groups().len(),
        selected_group_count: groups.len(),
        available_group_count: total_available,
        callable_group_count: *readiness_counts.get("callable").unwrap_or(&0),
        partial_group_count: *readiness_counts.get("partial").unwrap_or(&0),
        declared_only_group_count: *readiness_counts.get("declared_only").unwrap_or(&0),
        selected_tool_memberships,
        selected_unique_tools: selected_tools.len(),
        schema_backed_unique_tools: schema_backed_tools.len(),
        readiness_counts,
        gap_counts,
        groups,
        warnings,
        guarantees: vec![
            "group ordering and dashboard digests are deterministic for the same catalogue, schema map, and query".into(),
            "callable means only that an authoritative MCP schema is present; it is not permission, scientific validity, or execution success".into(),
            "CLI, Python, crate, and MCP surfaces remain separate counts rather than a blended availability score".into(),
        ],
        limitations: vec![
            "the dashboard does not execute tools, inspect external installations, authenticate users, or verify that a declared CLI or Python artifact is importable".into(),
            "catalogue labels and surface declarations remain caller-maintained workspace metadata".into(),
            "a bounded dashboard must not be read as a complete domain inventory without checking warnings and max_groups".into(),
        ],
        ready,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn catalogue() -> CapabilityCatalogue {
        CapabilityCatalogue::from_value(&json!([
            {"id":"alpha","domains":["science"],"crates":["a"],"mcp_tools":["tool_a","missing","invalid"],"cli_entrypoints":["a"],"python_artifacts":["python/a"],"status":"available"},
            {"id":"beta","domains":["operations"],"crates":["b"],"mcp_tools":[],"cli_entrypoints":[],"python_artifacts":[],"status":"planned"}
        ]))
        .unwrap()
    }

    #[test]
    fn dashboard_classifies_callable_partial_and_gaps_deterministically() {
        let mut schemas = BTreeMap::new();
        schemas.insert("tool_a".into(), true);
        schemas.insert("invalid".into(), false);
        let report =
            build_dashboard(&catalogue(), &schemas, &CapabilityDashboardQuery::default()).unwrap();
        assert_eq!(report.groups[0].id, "alpha");
        assert_eq!(report.groups[0].readiness, "partial");
        assert_eq!(report.groups[0].callable_tool_count, 1);
        assert_eq!(report.groups[0].schema_backed_tool_count, 1);
        assert_eq!(report.groups[0].invalid_transport_schemas, vec!["invalid"]);
        assert_eq!(report.groups[1].readiness, "declared_only");
        assert_eq!(report.gap_counts["missing_transport_schema"], 1);
        assert!(!report.ready);
    }

    #[test]
    fn dashboard_filters_and_bounds_are_explicit() {
        let query = CapabilityDashboardQuery {
            domain: Some("science".into()),
            max_groups: 1,
            ..Default::default()
        };
        let report = build_dashboard(
            &catalogue(),
            &BTreeMap::from([
                (String::from("tool_a"), true),
                (String::from("missing"), true),
            ]),
            &query,
        )
        .unwrap();
        assert_eq!(report.selected_group_count, 1);
        assert!(report.groups[0].tools.is_none());
        assert!(!report
            .warnings
            .iter()
            .any(|warning| warning.contains("bounded")));
        let bounded = build_dashboard(
            &catalogue(),
            &BTreeMap::from([
                (String::from("tool_a"), true),
                (String::from("missing"), true),
            ]),
            &CapabilityDashboardQuery {
                max_groups: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(bounded
            .warnings
            .iter()
            .any(|warning| warning.contains("bounded")));
        assert!(CapabilityDashboardQuery {
            max_groups: 0,
            ..Default::default()
        }
        .validate()
        .is_err());
    }
}
