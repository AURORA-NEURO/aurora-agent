//! Deterministic validation for a machine-readable engineering manifest.
//!
//! The manifest is deliberately an artifact contract, not an organisation simulator. It can
//! prove that package edges close, tickets name real packages and contracts, ADR supersession is
//! well formed, and ownership rows do not erase the independent-review boundary. It cannot prove
//! that a person performed a review, that a ticket was implemented, or that a CI runner, registry,
//! filesystem, or GitHub repository agrees with the document.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const ENGINEERING_MANIFEST_SCHEMA: &str = "bioprism-engineering-manifest/0.1";
pub const ENGINEERING_AUDIT_SCHEMA: &str = "bioprism-engineering-audit/0.1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringManifest {
    pub schema: String,
    pub project: ProjectIdentity,
    pub baseline: TechnologyBaseline,
    #[serde(default)]
    pub packages: Vec<PackageSpec>,
    #[serde(default)]
    pub tickets: Vec<TicketSpec>,
    #[serde(default)]
    pub adrs: Vec<AdrSpec>,
    #[serde(default)]
    pub ownership: Vec<OwnershipSpec>,
    #[serde(default)]
    pub policies: EngineeringPolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectIdentity {
    pub id: String,
    pub version: String,
    pub repository: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TechnologyBaseline {
    pub language: String,
    pub runtime: String,
    pub api: String,
    pub storage: String,
    pub observability: String,
    pub deployment: String,
    #[serde(default)]
    pub reasons: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageSpec {
    pub id: String,
    pub path: String,
    pub language: String,
    pub kind: String,
    pub owner: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub test_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TicketSpec {
    pub id: String,
    pub title: String,
    pub package: String,
    pub contract: String,
    pub status: TicketStatus,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub blocker: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TicketStatus {
    Planned,
    InProgress,
    Blocked,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdrSpec {
    pub id: String,
    pub title: String,
    pub status: AdrStatus,
    pub decision: String,
    pub affects: Vec<String>,
    #[serde(default)]
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdrStatus {
    Proposed,
    Accepted,
    Superseded,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OwnershipSpec {
    pub surface: String,
    pub accountable: String,
    pub responsible: Vec<String>,
    #[serde(default)]
    pub consulted: Vec<String>,
    #[serde(default)]
    pub informed: Vec<String>,
    #[serde(default)]
    pub independent_reviewer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringPolicies {
    #[serde(default = "default_true")]
    pub require_acyclic_packages: bool,
    #[serde(default = "default_true")]
    pub require_ticket_contracts: bool,
    #[serde(default = "default_true")]
    pub require_ownership: bool,
    #[serde(default = "default_true")]
    pub require_adr_targets: bool,
}

impl Default for EngineeringPolicies {
    fn default() -> Self {
        Self {
            require_acyclic_packages: true,
            require_ticket_contracts: true,
            require_ownership: true,
            require_adr_targets: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringIssue {
    pub code: String,
    pub severity: IssueSeverity,
    pub subject: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueSeverity {
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TicketReadiness {
    pub ticket_id: String,
    pub status: TicketStatus,
    pub state: String,
    pub blocking_dependencies: Vec<String>,
    pub dependency_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringCounts {
    pub packages: usize,
    pub public_packages: usize,
    pub tickets: usize,
    pub completed_tickets: usize,
    pub actionable_tickets: usize,
    pub adrs: usize,
    pub accepted_adrs: usize,
    pub ownership_rows: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringAudit {
    pub schema: String,
    pub manifest_schema: String,
    pub digest: String,
    pub valid: bool,
    pub counts: EngineeringCounts,
    pub package_order: Vec<String>,
    pub cyclic_packages: Vec<Vec<String>>,
    pub ticket_readiness: Vec<TicketReadiness>,
    pub adr_supersession: Vec<AdrSupersession>,
    pub ownership_surfaces: Vec<String>,
    pub issues: Vec<EngineeringIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdrSupersession {
    pub newer: String,
    pub older: String,
    pub valid: bool,
}

#[derive(Debug, Error)]
pub enum EngineeringError {
    #[error("cannot canonicalize engineering manifest: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize engineering manifest: {0}")]
    Serialization(String),
}

impl EngineeringManifest {
    pub fn digest(&self) -> Result<ContentHash, EngineeringError> {
        let value = serde_json::to_value(self)
            .map_err(|error| EngineeringError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<EngineeringAudit, EngineeringError> {
        let digest = self.digest()?.to_string();
        let mut issues = Vec::new();
        let mut package_ids = BTreeSet::new();
        let mut package_paths = BTreeMap::<String, String>::new();
        let mut package_map = BTreeMap::<String, &PackageSpec>::new();

        if self.schema != ENGINEERING_MANIFEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "manifest",
                format!("expected {ENGINEERING_MANIFEST_SCHEMA}, got {}", self.schema),
                "regenerate the manifest with the published schema",
            );
        }
        for (field, value) in [
            ("project.id", &self.project.id),
            ("project.version", &self.project.version),
            ("project.repository", &self.project.repository),
            ("baseline.language", &self.baseline.language),
            ("baseline.runtime", &self.baseline.runtime),
            ("baseline.api", &self.baseline.api),
            ("baseline.storage", &self.baseline.storage),
            ("baseline.observability", &self.baseline.observability),
            ("baseline.deployment", &self.baseline.deployment),
        ] {
            if value.trim().is_empty() {
                blocking(
                    &mut issues,
                    "required_field_empty",
                    field,
                    format!("{field} is empty"),
                    "supply a concrete value or refuse the manifest before publication",
                );
            }
        }

        for package in &self.packages {
            if !package_ids.insert(package.id.clone()) {
                blocking(
                    &mut issues,
                    "duplicate_package_id",
                    &package.id,
                    "the package identifier occurs more than once".to_string(),
                    "assign one stable identifier to exactly one package",
                );
            }
            if let Some(previous) = package_paths.insert(package.path.clone(), package.id.clone()) {
                blocking(
                    &mut issues,
                    "duplicate_package_path",
                    &package.path,
                    format!("packages {previous} and {} claim the same path", package.id),
                    "give every package an unambiguous repository-relative path",
                );
            }
            for (field, value) in [
                ("id", &package.id),
                ("path", &package.path),
                ("language", &package.language),
                ("kind", &package.kind),
                ("owner", &package.owner),
            ] {
                if value.trim().is_empty() {
                    blocking(
                        &mut issues,
                        "package_field_empty",
                        format!("package.{}.{}", package.id, field),
                        "a package field is empty".to_string(),
                        "complete the package identity before using it as a dependency target",
                    );
                }
            }
            package_map.insert(package.id.clone(), package);
        }

        let mut dependency_graph = BTreeMap::<String, Vec<String>>::new();
        for package in &self.packages {
            let mut dependencies = package.depends_on.clone();
            dependencies.sort();
            dependencies.dedup();
            for dependency in &dependencies {
                if dependency == &package.id {
                    blocking(
                        &mut issues,
                        "self_dependency",
                        &package.id,
                        "a package cannot depend on itself".to_string(),
                        "remove the self-edge or split the package boundary",
                    );
                } else if !package_map.contains_key(dependency) {
                    blocking(
                        &mut issues,
                        "missing_package_dependency",
                        &package.id,
                        format!("dependency {dependency} is not declared"),
                        "add the dependency to packages or remove the edge",
                    );
                }
            }
            dependency_graph.insert(package.id.clone(), dependencies);
        }

        let (package_order, cyclic_packages) = topo_order(&dependency_graph);
        if self.policies.require_acyclic_packages && !cyclic_packages.is_empty() {
            for cycle in &cyclic_packages {
                blocking(
                    &mut issues,
                    "package_cycle",
                    &cycle.join(" -> "),
                    "the package dependency graph contains a cycle".to_string(),
                    "break the cycle with an interface package or a one-way adapter",
                );
            }
        } else if !cyclic_packages.is_empty() {
            warning(
                &mut issues,
                "package_cycle_declared_allowed",
                "packages",
                "a cycle exists but policy explicitly permits it".to_string(),
                "keep the exception documented and revisit the boundary before release",
            );
        }

        let mut ticket_ids = BTreeSet::new();
        let mut ticket_map = BTreeMap::<String, &TicketSpec>::new();
        for ticket in &self.tickets {
            if !ticket_ids.insert(ticket.id.clone()) {
                blocking(
                    &mut issues,
                    "duplicate_ticket_id",
                    &ticket.id,
                    "the ticket identifier occurs more than once".to_string(),
                    "assign one stable identifier to exactly one ticket",
                );
            }
            for (field, value) in [
                ("id", &ticket.id),
                ("title", &ticket.title),
                ("package", &ticket.package),
                ("contract", &ticket.contract),
            ] {
                if value.trim().is_empty() {
                    blocking(
                        &mut issues,
                        "ticket_field_empty",
                        format!("ticket.{}.{}", ticket.id, field),
                        "a ticket identity or contract field is empty".to_string(),
                        "name the implementation surface and its checkable contract",
                    );
                }
            }
            if !package_map.contains_key(&ticket.package) {
                blocking(
                    &mut issues,
                    "ticket_package_missing",
                    &ticket.id,
                    format!("ticket names undeclared package {}", ticket.package),
                    "point the ticket at a declared package",
                );
            }
            if ticket.acceptance.is_empty() || ticket.acceptance.iter().any(|item| item.trim().is_empty()) {
                if self.policies.require_ticket_contracts {
                    blocking(
                        &mut issues,
                        "ticket_acceptance_missing",
                        &ticket.id,
                        "ticket must carry at least one non-empty acceptance condition".to_string(),
                        "write a testable acceptance condition or mark the work as unshaped",
                    );
                } else {
                    warning(
                        &mut issues,
                        "ticket_acceptance_missing",
                        &ticket.id,
                        "ticket has no complete acceptance condition".to_string(),
                        "add an observable acceptance condition before execution",
                    );
                }
            }
            if matches!(ticket.status, TicketStatus::Blocked)
                && ticket.blocker.as_deref().unwrap_or("").trim().is_empty()
            {
                blocking(
                    &mut issues,
                    "blocked_ticket_without_reason",
                    &ticket.id,
                    "blocked status has no named blocker".to_string(),
                    "state the dependency, decision, or external artifact that blocks the ticket",
                );
            }
            for dependency in &ticket.depends_on {
                if dependency == &ticket.id {
                    blocking(
                        &mut issues,
                        "ticket_self_dependency",
                        &ticket.id,
                        "a ticket cannot depend on itself".to_string(),
                        "remove the self-edge or split the ticket",
                    );
                } else if !ticket_ids.contains(dependency) {
                    // The complete ticket set is not known until the loop ends. This is checked again below.
                }
            }
            ticket_map.insert(ticket.id.clone(), ticket);
        }
        for ticket in &self.tickets {
            for dependency in &ticket.depends_on {
                if !ticket_map.contains_key(dependency) {
                    blocking(
                        &mut issues,
                        "missing_ticket_dependency",
                        &ticket.id,
                        format!("dependency {dependency} is not declared"),
                        "add the dependency or remove the edge",
                    );
                }
            }
        }

        let mut ticket_readiness = Vec::with_capacity(self.tickets.len());
        for ticket in &self.tickets {
            let blocking_dependencies = ticket
                .depends_on
                .iter()
                .filter(|dependency| {
                    ticket_map
                        .get(*dependency)
                        .map(|candidate| !matches!(candidate.status, TicketStatus::Done))
                        .unwrap_or(true)
                })
                .cloned()
                .collect::<Vec<_>>();
            let dependency_ready = blocking_dependencies.is_empty()
                && ticket.depends_on.iter().all(|dependency| ticket_map.contains_key(dependency));
            let state = match ticket.status {
                TicketStatus::Done => "complete",
                TicketStatus::Blocked => "blocked",
                TicketStatus::Planned | TicketStatus::InProgress if !dependency_ready => "waiting",
                TicketStatus::Planned | TicketStatus::InProgress => "actionable",
            };
            ticket_readiness.push(TicketReadiness {
                ticket_id: ticket.id.clone(),
                status: ticket.status.clone(),
                state: state.to_string(),
                blocking_dependencies,
                dependency_ready,
            });
        }

        let mut adr_ids = BTreeSet::new();
        let mut adr_map = BTreeMap::<String, &AdrSpec>::new();
        for adr in &self.adrs {
            if !adr_ids.insert(adr.id.clone()) {
                blocking(
                    &mut issues,
                    "duplicate_adr_id",
                    &adr.id,
                    "the ADR identifier occurs more than once".to_string(),
                    "assign one stable identifier to exactly one decision record",
                );
            }
            for (field, value) in [
                ("id", &adr.id),
                ("title", &adr.title),
                ("decision", &adr.decision),
            ] {
                if value.trim().is_empty() {
                    blocking(
                        &mut issues,
                        "adr_field_empty",
                        format!("adr.{}.{}", adr.id, field),
                        "an ADR identity or decision field is empty".to_string(),
                        "preserve the decision and its reason as part of the record",
                    );
                }
            }
            if adr.affects.is_empty() || adr.affects.iter().any(|item| item.trim().is_empty()) {
                if self.policies.require_adr_targets {
                    blocking(
                        &mut issues,
                        "adr_target_missing",
                        &adr.id,
                        "an ADR must name at least one affected surface".to_string(),
                        "name the package, contract, or public surface changed by the decision",
                    );
                }
            }
            adr_map.insert(adr.id.clone(), adr);
        }
        let mut adr_supersession = Vec::new();
        for adr in &self.adrs {
            if let Some(older) = &adr.supersedes {
                let valid = older != &adr.id && adr_map.contains_key(older);
                adr_supersession.push(AdrSupersession {
                    newer: adr.id.clone(),
                    older: older.clone(),
                    valid,
                });
                if !valid {
                    blocking(
                        &mut issues,
                        "invalid_adr_supersession",
                        &adr.id,
                        format!("superseded ADR {older} is absent or self-referential"),
                        "reference an earlier ADR in the same manifest",
                    );
                }
            }
        }
        if let Some(cycle) = supersession_cycle(&adr_map) {
            blocking(
                &mut issues,
                "adr_supersession_cycle",
                &cycle.join(" -> "),
                "ADR supersession must form a history, not a cycle".to_string(),
                "retain one terminal decision and point newer records at it",
            );
        }

        let mut ownership_surfaces = Vec::new();
        let mut seen_surfaces = BTreeSet::new();
        for row in &self.ownership {
            if !seen_surfaces.insert(row.surface.clone()) {
                blocking(
                    &mut issues,
                    "duplicate_ownership_surface",
                    &row.surface,
                    "the surface has more than one RACI row".to_string(),
                    "merge the row or give each independently governed surface its own name",
                );
            }
            ownership_surfaces.push(row.surface.clone());
            if row.surface.trim().is_empty()
                || row.accountable.trim().is_empty()
                || row.responsible.is_empty()
                || row.responsible.iter().any(|person| person.trim().is_empty())
            {
                if self.policies.require_ownership {
                    blocking(
                        &mut issues,
                        "ownership_row_incomplete",
                        &row.surface,
                        "a RACI row needs a surface, one accountable party, and one responsible party".to_string(),
                        "complete the ownership row before treating it as a boundary",
                    );
                }
            }
            if let Some(reviewer) = &row.independent_reviewer {
                if reviewer == &row.accountable || row.responsible.iter().any(|person| person == reviewer) {
                    blocking(
                        &mut issues,
                        "reviewer_not_independent",
                        &row.surface,
                        "the named independent reviewer is also accountable or responsible".to_string(),
                        "name a reviewer outside the authoring and accountable roles",
                    );
                }
            }
        }
        ownership_surfaces.sort();

        let completed_tickets = self
            .tickets
            .iter()
            .filter(|ticket| matches!(ticket.status, TicketStatus::Done))
            .count();
        let actionable_tickets = ticket_readiness
            .iter()
            .filter(|ticket| ticket.state == "actionable")
            .count();
        let counts = EngineeringCounts {
            packages: self.packages.len(),
            public_packages: self.packages.iter().filter(|package| package.public).count(),
            tickets: self.tickets.len(),
            completed_tickets,
            actionable_tickets,
            adrs: self.adrs.len(),
            accepted_adrs: self
                .adrs
                .iter()
                .filter(|adr| matches!(adr.status, AdrStatus::Accepted))
                .count(),
            ownership_rows: self.ownership.len(),
        };
        let valid = !issues
            .iter()
            .any(|issue| matches!(issue.severity, IssueSeverity::Blocking));
        Ok(EngineeringAudit {
            schema: ENGINEERING_AUDIT_SCHEMA.to_string(),
            manifest_schema: self.schema.clone(),
            digest,
            valid,
            counts,
            package_order,
            cyclic_packages,
            ticket_readiness,
            adr_supersession,
            ownership_surfaces,
            issues,
            guarantees: vec![
                "the digest binds the canonical manifest, not an external checkout".to_string(),
                "package and ticket dependency edges are checked separately and cycles remain visible".to_string(),
                "ticket readiness is dependency posture, not proof that work was implemented".to_string(),
                "ADR supersession and ownership separation are explicit rather than inferred".to_string(),
            ],
            limitations: vec![
                "the audit does not run tests, CI, workflows, or ticket systems".to_string(),
                "package paths and owners are caller-declared and are not read from disk".to_string(),
                "a valid manifest is an internally coherent plan, not an approval or release decision".to_string(),
            ],
        })
    }
}

fn blocking(
    issues: &mut Vec<EngineeringIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(EngineeringIssue {
        code: code.to_string(),
        severity: IssueSeverity::Blocking,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

fn warning(
    issues: &mut Vec<EngineeringIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(EngineeringIssue {
        code: code.to_string(),
        severity: IssueSeverity::Warning,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

fn topo_order(graph: &BTreeMap<String, Vec<String>>) -> (Vec<String>, Vec<Vec<String>>) {
    let mut incoming = graph
        .keys()
        .map(|key| (key.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, Vec<String>>::new();
    for (node, dependencies) in graph {
        for dependency in dependencies {
            if graph.contains_key(dependency) {
                *incoming.entry(node.clone()).or_default() += 1;
                outgoing.entry(dependency.clone()).or_default().push(node.clone());
            }
        }
    }
    for values in outgoing.values_mut() {
        values.sort();
    }
    let mut ready = incoming
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::new();
    while let Some(node) = ready.pop_first() {
        order.push(node.clone());
        for dependent in outgoing.get(&node).into_iter().flatten() {
            let degree = incoming.get_mut(dependent).expect("outgoing target exists");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    let remaining = incoming
        .iter()
        .filter(|(_, degree)| **degree > 0)
        .map(|(node, _)| node.clone())
        .collect::<BTreeSet<_>>();
    let cycles = if remaining.is_empty() {
        Vec::new()
    } else {
        connected_cycles(graph, &remaining)
    };
    (order, cycles)
}

fn connected_cycles(graph: &BTreeMap<String, Vec<String>>, remaining: &BTreeSet<String>) -> Vec<Vec<String>> {
    let mut undirected = BTreeMap::<String, BTreeSet<String>>::new();
    for node in remaining {
        for dependency in graph.get(node).into_iter().flatten() {
            if remaining.contains(dependency) {
                undirected.entry(node.clone()).or_default().insert(dependency.clone());
                undirected.entry(dependency.clone()).or_default().insert(node.clone());
            }
        }
    }
    let mut seen = BTreeSet::new();
    let mut components = Vec::new();
    for node in remaining {
        if !seen.insert(node.clone()) {
            continue;
        }
        let mut stack = vec![node.clone()];
        let mut component = Vec::new();
        while let Some(current) = stack.pop() {
            component.push(current.clone());
            for neighbor in undirected.get(&current).into_iter().flatten() {
                if seen.insert(neighbor.clone()) {
                    stack.push(neighbor.clone());
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components
}

fn supersession_cycle(adrs: &BTreeMap<String, &AdrSpec>) -> Option<Vec<String>> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = Vec::new();
    for id in adrs.keys() {
        if let Some(cycle) = visit_adr(id, adrs, &mut visiting, &mut visited, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

fn visit_adr(
    id: &str,
    adrs: &BTreeMap<String, &AdrSpec>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
) -> Option<Vec<String>> {
    if visited.contains(id) {
        return None;
    }
    if !visiting.insert(id.to_string()) {
        let start = stack.iter().position(|item| item == id).unwrap_or(0);
        return Some(stack[start..].iter().cloned().chain([id.to_string()]).collect());
    }
    stack.push(id.to_string());
    if let Some(Some(next)) = adrs.get(id).map(|adr| adr.supersedes.as_ref()) {
        if adrs.contains_key(next) {
            if let Some(cycle) = visit_adr(next, adrs, visiting, visited, stack) {
                return Some(cycle);
            }
        }
    }
    stack.pop();
    visiting.remove(id);
    visited.insert(id.to_string());
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> EngineeringManifest {
        EngineeringManifest {
            schema: ENGINEERING_MANIFEST_SCHEMA.to_string(),
            project: ProjectIdentity {
                id: "aurora-agent".into(),
                version: "0.1.0".into(),
                repository: "github.com/AURORA-NEURO/aurora-agent".into(),
            },
            baseline: TechnologyBaseline {
                language: "Rust 2021".into(),
                runtime: "cargo".into(),
                api: "MCP JSON-RPC".into(),
                storage: "in-memory".into(),
                observability: "structured stderr audit".into(),
                deployment: "local process".into(),
                reasons: BTreeMap::new(),
            },
            packages: vec![
                PackageSpec {
                    id: "core".into(),
                    path: "crates/core".into(),
                    language: "rust".into(),
                    kind: "library".into(),
                    owner: "platform".into(),
                    depends_on: vec![],
                    public: true,
                    test_command: Some("cargo test -p core".into()),
                },
                PackageSpec {
                    id: "api".into(),
                    path: "crates/api".into(),
                    language: "rust".into(),
                    kind: "service".into(),
                    owner: "platform".into(),
                    depends_on: vec!["core".into()],
                    public: true,
                    test_command: Some("cargo test -p api".into()),
                },
            ],
            tickets: vec![
                TicketSpec {
                    id: "T-001".into(),
                    title: "ship core".into(),
                    package: "core".into(),
                    contract: "core-contract".into(),
                    status: TicketStatus::Done,
                    depends_on: vec![],
                    acceptance: vec!["core tests pass".into()],
                    blocker: None,
                },
                TicketSpec {
                    id: "T-002".into(),
                    title: "ship api".into(),
                    package: "api".into(),
                    contract: "api-contract".into(),
                    status: TicketStatus::Planned,
                    depends_on: vec!["T-001".into()],
                    acceptance: vec!["protocol tests pass".into()],
                    blocker: None,
                },
            ],
            adrs: vec![AdrSpec {
                id: "ADR-001".into(),
                title: "use rust".into(),
                status: AdrStatus::Accepted,
                decision: "Rust owns canonical semantics".into(),
                affects: vec!["core".into(), "api".into()],
                supersedes: None,
            }],
            ownership: vec![OwnershipSpec {
                surface: "api".into(),
                accountable: "platform-lead".into(),
                responsible: vec!["api-team".into()],
                consulted: vec!["security".into()],
                informed: vec!["research".into()],
                independent_reviewer: Some("review-board".into()),
            }],
            policies: EngineeringPolicies::default(),
        }
    }

    #[test]
    fn valid_manifest_has_deterministic_order_and_actionable_ticket() {
        let report = manifest().audit().expect("valid manifest");
        assert!(report.valid);
        assert_eq!(report.package_order, vec!["core", "api"]);
        assert_eq!(report.ticket_readiness[1].state, "actionable");
        assert_eq!(report.counts.actionable_tickets, 1);
        assert_eq!(report.digest, manifest().audit().unwrap().digest);
    }

    #[test]
    fn package_cycles_and_missing_edges_are_blocking_and_visible() {
        let mut value = manifest();
        value.packages[0].depends_on = vec!["api".into(), "missing".into()];
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "package_cycle"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "missing_package_dependency"));
        assert_eq!(report.cyclic_packages, vec![vec!["api", "core"]]);
    }

    #[test]
    fn adr_and_ownership_boundaries_refuse_self_reference() {
        let mut value = manifest();
        value.adrs = vec![
            AdrSpec {
                id: "ADR-001".into(),
                title: "one".into(),
                status: AdrStatus::Accepted,
                decision: "first".into(),
                affects: vec!["core".into()],
                supersedes: Some("ADR-002".into()),
            },
            AdrSpec {
                id: "ADR-002".into(),
                title: "two".into(),
                status: AdrStatus::Accepted,
                decision: "second".into(),
                affects: vec!["api".into()],
                supersedes: Some("ADR-001".into()),
            },
        ];
        value.ownership[0].independent_reviewer = Some("platform-lead".into());
        let report = value.audit().unwrap();
        assert!(report.issues.iter().any(|issue| issue.code == "adr_supersession_cycle"));
        assert!(report.issues.iter().any(|issue| issue.code == "reviewer_not_independent"));
    }

    #[test]
    fn serialized_manifest_round_trips_without_losing_policy() {
        let value = manifest();
        let encoded = serde_json::to_value(&value).unwrap();
        let decoded: EngineeringManifest = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, value);
        assert!(decoded.policies.require_ticket_contracts);
    }
}
