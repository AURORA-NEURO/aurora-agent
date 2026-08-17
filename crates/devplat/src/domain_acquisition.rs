//! Cross-domain acquisition and adapter-conformance routing.
//!
//! The workspace capability catalogue says which tools are declared for a group, while the
//! adapter registry says which physical formats can be interpreted. Those are different facts.
//! This module joins them without collapsing them into an inflated "supported" boolean:
//!
//! * transport describes whether a domain has the bounded file/HTTP source-plan and intake
//!   seam, or only a caller-managed/declared intake surface;
//! * interpretation describes native and Python-delegated adapter routes whose *declared scope
//!   labels* overlap the domain label;
//! * every route carries its basis and limitations, so a lexical scope match is never presented
//!   as ontology validation, scientific validity, or provider execution.
//!
//! The result is deliberately pure. It consumes the authoritative capability catalogue and the
//! Rust adapter registry, performs no I/O, imports no Python packages, and grants no credentials.

use bioprism_adapter::{AdapterExecution, AdapterRegistry, ConformanceLevel};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const DOMAIN_ACQUISITION_SCHEMA_VERSION: &str = "bioprism-devplat-domain-acquisition/0.1";
pub const DOMAIN_ACQUISITION_WORKFLOW: &str = "domain_acquisition_catalogue";
pub const MAX_DOMAIN_ACQUISITION_GROUPS: usize = 64;
pub const MAX_DOMAIN_ACQUISITION_DOMAINS: usize = 512;
pub const MAX_DOMAIN_ACQUISITION_ADAPTERS: usize = 64;

const BOUNDED_TRANSPORT_TOOLS: &[&str] = &[
    "domain_evidence_source_plan",
    "domain_evidence_source_execute",
    "domain_evidence_intake",
];
const CALLER_MANAGED_CONNECTORS: &[&str] = &[
    "literature",
    "clinical_trial",
    "fhir",
    "object_store",
    "provider_api",
];

fn default_max_groups() -> usize {
    MAX_DOMAIN_ACQUISITION_GROUPS
}

fn default_max_domains() -> usize {
    MAX_DOMAIN_ACQUISITION_DOMAINS
}

/// Bounded filters for the cross-domain acquisition report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainAcquisitionQuery {
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
    /// Include the full adapter route descriptors, not just their stable IDs.
    #[serde(default)]
    pub include_adapters: bool,
    #[serde(default = "default_max_groups")]
    pub max_groups: usize,
    #[serde(default = "default_max_domains")]
    pub max_domains: usize,
}

impl Default for DomainAcquisitionQuery {
    fn default() -> Self {
        Self {
            group_id: None,
            domain: None,
            include_adapters: false,
            max_groups: MAX_DOMAIN_ACQUISITION_GROUPS,
            max_domains: MAX_DOMAIN_ACQUISITION_DOMAINS,
        }
    }
}

impl DomainAcquisitionQuery {
    pub fn validate(&self) -> Result<(), DomainAcquisitionError> {
        validate_filter("group_id", &self.group_id)?;
        validate_filter("domain", &self.domain)?;
        if !(1..=MAX_DOMAIN_ACQUISITION_GROUPS).contains(&self.max_groups) {
            return Err(DomainAcquisitionError::InvalidLimit {
                field: "max_groups",
                value: self.max_groups,
                maximum: MAX_DOMAIN_ACQUISITION_GROUPS,
            });
        }
        if !(1..=MAX_DOMAIN_ACQUISITION_DOMAINS).contains(&self.max_domains) {
            return Err(DomainAcquisitionError::InvalidLimit {
                field: "max_domains",
                value: self.max_domains,
                maximum: MAX_DOMAIN_ACQUISITION_DOMAINS,
            });
        }
        Ok(())
    }
}

/// The independently auditable transport half of a domain route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainTransportRoute {
    /// `bounded_file_http`, `caller_managed_plan`, `caller_supplied_intake`, or `none`.
    pub status: String,
    pub tools: Vec<String>,
    pub bounded_connector_kinds: Vec<String>,
    pub caller_managed_connector_kinds: Vec<String>,
    pub limitations: Vec<String>,
}

/// The independently auditable interpretation half of a domain route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainInterpretationRoute {
    /// `native`, `python_delegated`, `mixed`, `domain_tools_only`, or `unmapped`.
    pub status: String,
    pub adapter_ids: Vec<String>,
    pub match_basis: Vec<String>,
    pub declared_conformance: Vec<String>,
    pub limitations: Vec<String>,
}

/// A detailed adapter route included when `include_adapters` is requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainAdapterRoute {
    pub id: String,
    pub execution: String,
    pub version: String,
    pub accepted_formats: Vec<String>,
    pub source_kinds: Vec<String>,
    pub conformance_level: String,
    pub optional_dependency: Option<String>,
    pub scope_dimensions: Vec<String>,
    pub match_basis: Vec<String>,
}

/// One declared domain and its two-plane acquisition/conformance route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainAcquisitionRoute {
    pub group_id: String,
    pub domain: String,
    pub declared_tool_count: usize,
    pub transport: DomainTransportRoute,
    pub interpretation: DomainInterpretationRoute,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapters: Option<Vec<DomainAdapterRoute>>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

/// Group-level summary retaining the relationship between the catalogue and domain rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainAcquisitionGroup {
    pub id: String,
    pub status: String,
    pub declared_domain_count: usize,
    pub selected_domain_count: usize,
    pub declared_tool_count: usize,
    pub transport_status: String,
    pub interpretation_statuses: Vec<String>,
}

/// Deterministic, digest-bound report over the selected catalogue slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainAcquisitionCatalogue {
    pub schema: String,
    pub workflow: String,
    pub catalogue_digest: String,
    pub adapter_registry: String,
    pub adapter_registry_digest: String,
    pub query: DomainAcquisitionQuery,
    pub total_group_count: usize,
    pub selected_group_count: usize,
    pub total_domain_count: usize,
    pub selected_domain_count: usize,
    pub complete: bool,
    pub truncated: bool,
    pub groups: Vec<DomainAcquisitionGroup>,
    pub routes: Vec<DomainAcquisitionRoute>,
    pub warnings: Vec<String>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
    pub digest: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainAcquisitionError {
    #[error("{field} filter must be non-empty when supplied")]
    EmptyFilter { field: &'static str },
    #[error("{field} filter exceeds the 512-byte safety bound")]
    FilterTooLong { field: &'static str },
    #[error("{field} filter contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("{field} must be between 1 and {maximum}, got {value}")]
    InvalidLimit {
        field: &'static str,
        value: usize,
        maximum: usize,
    },
    #[error("could not canonicalize domain acquisition catalogue: {0}")]
    Canonical(String),
}

/// Build the report from the authoritative capability and adapter registries.
pub fn build_domain_acquisition_catalogue(
    catalogue: &crate::CapabilityCatalogue,
    adapters: &AdapterRegistry,
    query: &DomainAcquisitionQuery,
) -> Result<DomainAcquisitionCatalogue, DomainAcquisitionError> {
    query.validate()?;
    let adapter_registry_value = serde_json::to_value(adapters.descriptors())
        .map_err(|error| DomainAcquisitionError::Canonical(error.to_string()))?;
    let adapter_registry_digest = ContentHash::of_value(&adapter_registry_value)
        .map_err(|error| DomainAcquisitionError::Canonical(error.to_string()))?
        .to_string();

    let group_filter = query.group_id.as_deref().map(normalized);
    let domain_filter = query.domain.as_deref().map(normalized);
    let mut selected_groups = catalogue
        .groups()
        .iter()
        .filter(|group| {
            group_filter.as_ref().is_none_or(|filter| {
                let id = normalized(&group.id);
                id == *filter || id.starts_with(filter)
            })
        })
        .filter(|group| {
            domain_filter.as_ref().is_none_or(|filter| {
                group
                    .domains
                    .iter()
                    .any(|domain| normalized(domain).contains(filter))
            })
        })
        .collect::<Vec<_>>();
    selected_groups.sort_by(|left, right| left.id.cmp(&right.id));
    let total_domain_count = selected_groups
        .iter()
        .map(|group| {
            group
                .domains
                .iter()
                .filter(|domain| {
                    domain_filter
                        .as_ref()
                        .is_none_or(|filter| normalized(domain).contains(filter))
                })
                .count()
        })
        .sum::<usize>();
    let truncated_groups = selected_groups.len() > query.max_groups;
    selected_groups.truncate(query.max_groups);
    let mut groups = Vec::new();
    let mut routes = Vec::new();
    let mut truncated_domains = false;
    for group in selected_groups {
        let domains = group
            .domains
            .iter()
            .filter(|domain| {
                domain_filter
                    .as_ref()
                    .is_none_or(|filter| normalized(domain).contains(filter))
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut selected_domain_count = 0;
        for domain in domains {
            if routes.len() >= query.max_domains {
                truncated_domains = true;
                break;
            }
            selected_domain_count += 1;
            routes.push(build_route(
                group,
                &domain,
                adapters,
                query.include_adapters,
            ));
        }
        let transport_status = transport_status(&group.mcp_tools);
        let interpretation_statuses = routes
            .iter()
            .filter(|route| route.group_id == group.id)
            .map(|route| route.interpretation.status.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        groups.push(DomainAcquisitionGroup {
            id: group.id.clone(),
            status: group.status.clone(),
            declared_domain_count: group.domains.len(),
            selected_domain_count,
            declared_tool_count: group.mcp_tools.len(),
            transport_status,
            interpretation_statuses,
        });
        if truncated_domains {
            break;
        }
    }

    let truncated = truncated_groups || truncated_domains;
    let mut warnings = Vec::new();
    if groups.is_empty() {
        warnings.push("no capability groups matched the acquisition query".into());
    }
    if truncated_groups {
        warnings.push(
            "group output is bounded; increase max_groups before claiming catalogue completeness"
                .into(),
        );
    }
    if truncated_domains {
        warnings.push(
            "domain output is bounded; increase max_domains before claiming domain completeness"
                .into(),
        );
    }
    let mut report = DomainAcquisitionCatalogue {
        schema: DOMAIN_ACQUISITION_SCHEMA_VERSION.into(),
        workflow: DOMAIN_ACQUISITION_WORKFLOW.into(),
        catalogue_digest: catalogue.digest().to_string(),
        adapter_registry: bioprism_adapter::ADAPTER_REGISTRY_SCHEMA_VERSION.into(),
        adapter_registry_digest,
        query: query.clone(),
        total_group_count: catalogue.groups().len(),
        selected_group_count: groups.len(),
        total_domain_count,
        selected_domain_count: routes.len(),
        complete: !truncated && groups.len() == selected_group_count(catalogue, query),
        truncated,
        groups,
        routes,
        warnings,
        guarantees: vec![
            "transport and interpretation are reported as separate planes".into(),
            "adapter matches are based only on declared scope-label overlap and are not ontology resolution".into(),
            "all route ordering and the report digest are deterministic for the same registries and query".into(),
        ],
        limitations: vec![
            "transport coverage does not prove source authenticity, provider execution, or response truth".into(),
            "native and Python-delegated adapter declarations require source-specific conformance before facts are publishable".into(),
            "caller-managed connectors still require an external integration and are not executed by this report".into(),
        ],
        digest: String::new(),
    };
    let digest_input = serde_json::to_value((
        &report.catalogue_digest,
        &report.adapter_registry_digest,
        &report.query,
        &report.groups,
        &report.routes,
        &report.warnings,
    ))
    .map_err(|error| DomainAcquisitionError::Canonical(error.to_string()))?;
    report.digest = ContentHash::of_value(&digest_input)
        .map_err(|error| DomainAcquisitionError::Canonical(error.to_string()))?
        .to_string();
    Ok(report)
}

fn selected_group_count(
    catalogue: &crate::CapabilityCatalogue,
    query: &DomainAcquisitionQuery,
) -> usize {
    let group_filter = query.group_id.as_deref().map(normalized);
    let domain_filter = query.domain.as_deref().map(normalized);
    catalogue
        .groups()
        .iter()
        .filter(|group| {
            group_filter.as_ref().is_none_or(|filter| {
                let id = normalized(&group.id);
                id == *filter || id.starts_with(filter)
            })
        })
        .filter(|group| {
            domain_filter.as_ref().is_none_or(|filter| {
                group
                    .domains
                    .iter()
                    .any(|domain| normalized(domain).contains(filter))
            })
        })
        .count()
}

fn build_route(
    group: &crate::CapabilityGroup,
    domain: &str,
    adapters: &AdapterRegistry,
    include_adapters: bool,
) -> DomainAcquisitionRoute {
    let tools = BOUNDED_TRANSPORT_TOOLS
        .iter()
        .filter(|tool| group.mcp_tools.iter().any(|candidate| candidate == **tool))
        .map(|tool| (*tool).to_string())
        .collect::<Vec<_>>();
    let transport = DomainTransportRoute {
        status: transport_status(&group.mcp_tools),
        tools,
        bounded_connector_kinds: vec!["file".into(), "generic_http".into()],
        caller_managed_connector_kinds: CALLER_MANAGED_CONNECTORS
            .iter()
            .map(|kind| (*kind).to_string())
            .collect(),
        limitations: vec![
            "generic_http is plain-HTTP only in the in-process kernel; HTTPS and redirects remain refused".into(),
            "literature, clinical-trial, FHIR, object-store, and provider connectors remain caller-managed".into(),
        ],
    };
    let matched = adapters
        .descriptors()
        .iter()
        .filter(|adapter| {
            adapter
                .scope_dimensions
                .iter()
                .any(|dimension| scope_labels_overlap(domain, dimension))
        })
        .take(MAX_DOMAIN_ACQUISITION_ADAPTERS)
        .collect::<Vec<_>>();
    let has_native = matched
        .iter()
        .any(|adapter| adapter.execution == AdapterExecution::Native);
    let has_delegated = matched
        .iter()
        .any(|adapter| adapter.execution == AdapterExecution::PythonDelegated);
    let interpretation_status = if has_native && has_delegated {
        "mixed"
    } else if has_native {
        "native"
    } else if has_delegated {
        "python_delegated"
    } else if !group.mcp_tools.is_empty() {
        "domain_tools_only"
    } else {
        "unmapped"
    };
    let adapter_ids = matched.iter().map(|adapter| adapter.id.clone()).collect();
    let declared_conformance = matched
        .iter()
        .map(|adapter| conformance_name(adapter.conformance_level).to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let interpretation = DomainInterpretationRoute {
        status: interpretation_status.into(),
        adapter_ids,
        match_basis: matched
            .iter()
            .map(|adapter| {
                format!(
                    "declared scope overlap: {}",
                    adapter.scope_dimensions.iter().cloned().collect::<Vec<_>>().join(", ")
                )
            })
            .collect(),
        declared_conformance,
        limitations: vec![
            "scope-label overlap is a routing hint, not semantic or ontology validation".into(),
            "adapter execution, optional dependencies, and source-specific loss audits remain separate steps".into(),
        ],
    };
    let adapters = include_adapters.then(|| {
        matched
            .iter()
            .map(|adapter| DomainAdapterRoute {
                id: adapter.id.clone(),
                execution: execution_name(adapter.execution).into(),
                version: adapter.version.clone(),
                accepted_formats: adapter.accepted_formats.clone(),
                source_kinds: adapter
                    .source_kinds
                    .iter()
                    .map(|kind| kind.as_str().to_string())
                    .collect(),
                conformance_level: conformance_name(adapter.conformance_level).into(),
                optional_dependency: adapter.optional_dependency.clone(),
                scope_dimensions: adapter.scope_dimensions.iter().cloned().collect(),
                match_basis: vec!["declared scope-label overlap".into()],
            })
            .collect()
    });
    DomainAcquisitionRoute {
        group_id: group.id.clone(),
        domain: domain.to_string(),
        declared_tool_count: group.mcp_tools.len(),
        transport,
        interpretation,
        adapters,
        guarantees: vec![
            "the domain label is copied from the authoritative capability catalogue".into(),
            "transport tools are reported only when they are declared for this group".into(),
        ],
        limitations: vec![
            "a declared tool is not evidence that its external source was called".into(),
            "a matched adapter is not evidence that bytes were parsed or normalized".into(),
        ],
    }
}

fn transport_status(tools: &[String]) -> String {
    let has = |tool: &str| tools.iter().any(|candidate| candidate == tool);
    if BOUNDED_TRANSPORT_TOOLS.iter().all(|tool| has(tool)) {
        "bounded_file_http".into()
    } else if has("domain_evidence_source_plan") || has("domain_evidence_intake") {
        "caller_managed_plan".into()
    } else {
        "none".into()
    }
}

fn validate_filter(
    field: &'static str,
    value: &Option<String>,
) -> Result<(), DomainAcquisitionError> {
    let Some(value) = value else { return Ok(()) };
    if value.trim().is_empty() {
        return Err(DomainAcquisitionError::EmptyFilter { field });
    }
    if value.len() > 512 {
        return Err(DomainAcquisitionError::FilterTooLong { field });
    }
    if value
        .chars()
        .any(|character| character == '\0' || character == '\n' || character == '\r')
    {
        return Err(DomainAcquisitionError::ControlCharacter { field });
    }
    Ok(())
}

fn normalized(value: &str) -> String {
    value.to_ascii_lowercase()
}

fn tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn scope_labels_overlap(domain: &str, dimension: &str) -> bool {
    let domain_tokens = tokens(domain);
    let dimension_tokens = tokens(dimension);
    domain_tokens
        .iter()
        .any(|token| dimension_tokens.contains(token))
}

fn execution_name(execution: AdapterExecution) -> &'static str {
    match execution {
        AdapterExecution::Native => "native",
        AdapterExecution::PythonDelegated => "python_delegated",
    }
}

fn conformance_name(level: ConformanceLevel) -> &'static str {
    match level {
        ConformanceLevel::Parse => "parse",
        ConformanceLevel::Normalize => "normalize",
        ConformanceLevel::Execute => "execute",
        ConformanceLevel::Stream => "stream",
        ConformanceLevel::Replay => "replay",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CapabilityCatalogue;
    use serde_json::json;

    fn catalogue() -> CapabilityCatalogue {
        CapabilityCatalogue::from_value(&json!([
            {"id":"alpha","domains":["specimen lineage","custom science"],"mcp_tools":["domain_evidence_source_plan","domain_evidence_source_execute","domain_evidence_intake"],"status":"available"},
            {"id":"beta","domains":["operations"],"mcp_tools":["quality_gate_run"],"status":"available"}
        ]))
        .unwrap()
    }

    #[test]
    fn separates_transport_from_scope_matched_interpretation() {
        let report = build_domain_acquisition_catalogue(
            &catalogue(),
            &AdapterRegistry::default(),
            &DomainAcquisitionQuery {
                include_adapters: true,
                ..Default::default()
            },
        )
        .unwrap();
        let specimen = report
            .routes
            .iter()
            .find(|route| route.domain == "specimen lineage")
            .unwrap();
        assert_eq!(specimen.transport.status, "bounded_file_http");
        assert!(!specimen.interpretation.adapter_ids.is_empty());
        assert!(specimen
            .adapters
            .as_ref()
            .unwrap()
            .iter()
            .any(|adapter| adapter.id == "bioprism.tabular"));
        let custom = report
            .routes
            .iter()
            .find(|route| route.domain == "custom science")
            .unwrap();
        assert_eq!(custom.interpretation.status, "domain_tools_only");
    }

    #[test]
    fn filters_and_digest_are_deterministic() {
        let query = DomainAcquisitionQuery {
            group_id: Some("alp".into()),
            ..Default::default()
        };
        let first =
            build_domain_acquisition_catalogue(&catalogue(), &AdapterRegistry::default(), &query)
                .unwrap();
        let second =
            build_domain_acquisition_catalogue(&catalogue(), &AdapterRegistry::default(), &query)
                .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_group_count, 1);
        assert!(!first.digest.is_empty());
    }

    #[test]
    fn bounds_are_explicit() {
        let error = DomainAcquisitionQuery {
            max_domains: 0,
            ..Default::default()
        }
        .validate()
        .unwrap_err();
        assert!(error.to_string().contains("max_domains"));
    }
}
