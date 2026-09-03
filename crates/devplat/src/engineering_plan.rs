//! Deterministic execution planning over an [`EngineeringManifest`].
//!
//! The engineering manifest audit answers whether package, ticket, ADR, and ownership
//! declarations are internally coherent. This module adds the next useful layer for an agent:
//! it derives bounded dependency waves, per-ticket readiness, package-serialization decisions,
//! and a critical path without executing a ticket, reading a checkout, contacting a tracker, or
//! claiming that a planned ticket was implemented.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::engineering::{
    EngineeringError, EngineeringIssue, EngineeringManifest, IssueSeverity, TicketSpec,
    TicketStatus,
};

pub const ENGINEERING_PLAN_REQUEST_SCHEMA: &str = "bioprism-engineering-plan/0.1";
pub const ENGINEERING_PLAN_AUDIT_SCHEMA: &str = "bioprism-engineering-plan-audit/0.1";
pub const MAX_PLAN_TICKETS: usize = 100;
pub const MAX_PLAN_PARALLELISM: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringPlanRequest {
    pub schema: String,
    pub manifest: EngineeringManifest,
    #[serde(default)]
    pub policies: EngineeringPlanPolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringPlanPolicies {
    #[serde(default = "default_true")]
    pub require_valid_manifest: bool,
    #[serde(default)]
    pub allow_truncation: bool,
    #[serde(default)]
    pub include_completed: bool,
    #[serde(default = "default_true")]
    pub serialize_same_package: bool,
    #[serde(default = "default_max_tickets")]
    pub max_tickets: usize,
    #[serde(default = "default_max_parallelism")]
    pub max_parallelism: usize,
}

impl Default for EngineeringPlanPolicies {
    fn default() -> Self {
        Self {
            require_valid_manifest: true,
            allow_truncation: false,
            include_completed: false,
            serialize_same_package: true,
            max_tickets: MAX_PLAN_TICKETS,
            max_parallelism: MAX_PLAN_PARALLELISM.min(16),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringTicketPlan {
    pub ticket_id: String,
    pub package: String,
    pub contract: String,
    pub status: TicketStatus,
    pub state: String,
    pub dependency_ids: Vec<String>,
    pub blocking_dependencies: Vec<String>,
    pub dependency_ready: bool,
    pub scheduled: bool,
    pub wave: Option<usize>,
    pub critical_path_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringPlanWave {
    pub index: usize,
    pub ticket_ids: Vec<String>,
    pub package_ids: Vec<String>,
    pub depends_on_waves: Vec<usize>,
    pub parallelism: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringPlanGate {
    pub name: String,
    pub passed: bool,
    pub required: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineeringPlanAudit {
    pub schema: String,
    pub request_schema: String,
    pub manifest_digest: String,
    pub plan_digest: String,
    pub valid: bool,
    pub planning_started: bool,
    pub truncated: bool,
    pub ticket_count: usize,
    pub planned_ticket_count: usize,
    pub omitted_ticket_count: usize,
    pub package_order: Vec<String>,
    pub ticket_plans: Vec<EngineeringTicketPlan>,
    pub waves: Vec<EngineeringPlanWave>,
    pub critical_path: Vec<String>,
    pub gates: Vec<EngineeringPlanGate>,
    pub manifest_issues: Vec<EngineeringIssue>,
    pub issues: Vec<EngineeringIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum EngineeringPlanError {
    #[error("cannot audit engineering manifest: {0}")]
    Manifest(#[from] EngineeringError),
    #[error("cannot canonicalize engineering plan: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize engineering plan: {0}")]
    Serialization(String),
}

impl EngineeringPlanRequest {
    pub fn digest(&self) -> Result<ContentHash, EngineeringPlanError> {
        let value = serde_json::to_value(self)
            .map_err(|error| EngineeringPlanError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<EngineeringPlanAudit, EngineeringPlanError> {
        let manifest_audit = self.manifest.audit()?;
        let manifest_digest = manifest_audit.digest.clone();
        let mut issues = Vec::new();

        if self.schema != ENGINEERING_PLAN_REQUEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "request",
                format!(
                    "expected {ENGINEERING_PLAN_REQUEST_SCHEMA}, got {}",
                    self.schema
                ),
                "regenerate the request with the published planning schema",
            );
        }
        if self.policies.max_tickets == 0 || self.policies.max_tickets > MAX_PLAN_TICKETS {
            blocking(
                &mut issues,
                "ticket_bound_invalid",
                "policies.max_tickets",
                format!("max_tickets must be between 1 and {MAX_PLAN_TICKETS}"),
                "choose a bounded ticket window",
            );
        }
        if self.policies.max_parallelism == 0
            || self.policies.max_parallelism > MAX_PLAN_PARALLELISM
        {
            blocking(
                &mut issues,
                "parallelism_bound_invalid",
                "policies.max_parallelism",
                format!("max_parallelism must be between 1 and {MAX_PLAN_PARALLELISM}"),
                "choose a bounded parallelism ceiling",
            );
        }
        if !manifest_audit.valid && self.policies.require_valid_manifest {
            blocking(
                &mut issues,
                "manifest_invalid",
                "manifest",
                "the engineering manifest contains blocking audit findings",
                "resolve the manifest findings before deriving an execution plan",
            );
        } else if !manifest_audit.valid {
            warning(
                &mut issues,
                "manifest_invalid_allowed",
                "manifest",
                "planning continues under an explicit invalid-manifest exception",
                "do not use this plan as delivery authorization until the manifest is valid",
            );
        }
        if manifest_audit
            .issues
            .iter()
            .any(|issue| issue.code == "ticket_cycle")
        {
            blocking(
                &mut issues,
                "ticket_cycle",
                "tickets",
                "a cyclic ticket dependency cannot produce a deterministic execution plan",
                "break the ticket cycle before scheduling any wave",
            );
        }

        let max_tickets = self.policies.max_tickets.clamp(1, MAX_PLAN_TICKETS);
        let max_parallelism = self.policies.max_parallelism.clamp(1, MAX_PLAN_PARALLELISM);
        let mut tickets = self.manifest.tickets.iter().collect::<Vec<_>>();
        tickets.sort_by(|left, right| left.id.cmp(&right.id));
        let ticket_count = tickets.len();
        let truncated = ticket_count > max_tickets;
        let omitted_ticket_count = ticket_count.saturating_sub(max_tickets);
        if truncated {
            if self.policies.allow_truncation {
                warning(
                    &mut issues,
                    "ticket_window_truncated",
                    "tickets",
                    format!(
                        "{} tickets are omitted from the bounded window of {max_tickets}",
                        omitted_ticket_count
                    ),
                    "plan the omitted tickets in a later bounded window before claiming a complete delivery plan",
                );
            } else {
                blocking(
                    &mut issues,
                    "ticket_window_truncated",
                    "tickets",
                    format!(
                        "{} tickets exceed the bounded planning window of {max_tickets}",
                        omitted_ticket_count
                    ),
                    "split the engineering manifest into bounded planning windows or explicitly allow truncation",
                );
            }
        }
        tickets.truncate(max_tickets);
        let selected = tickets
            .iter()
            .map(|ticket| (ticket.id.clone(), *ticket))
            .collect::<BTreeMap<_, _>>();
        let all_tickets = self
            .manifest
            .tickets
            .iter()
            .map(|ticket| (ticket.id.as_str(), ticket))
            .collect::<BTreeMap<_, _>>();
        let mut depths = BTreeMap::new();
        for id in selected.keys() {
            let mut visiting = BTreeSet::new();
            let depth = ticket_depth(id, &selected, &mut depths, &mut visiting);
            depths.insert(id.clone(), depth);
        }

        let mut plans = selected
            .iter()
            .map(|(id, ticket)| {
                let blocking_dependencies = blocking_dependencies(ticket, &all_tickets);
                let dependency_ready = blocking_dependencies.is_empty()
                    && ticket
                        .depends_on
                        .iter()
                        .all(|dependency| all_tickets.contains_key(dependency.as_str()));
                let state = ticket_state(ticket, dependency_ready);
                EngineeringTicketPlan {
                    ticket_id: id.clone(),
                    package: ticket.package.clone(),
                    contract: ticket.contract.clone(),
                    status: ticket.status.clone(),
                    state,
                    dependency_ids: sorted_dependencies(ticket),
                    blocking_dependencies,
                    dependency_ready,
                    scheduled: false,
                    wave: None,
                    critical_path_length: depths.get(id).copied().unwrap_or(1),
                }
            })
            .collect::<Vec<_>>();

        let planning_started = self.schema == ENGINEERING_PLAN_REQUEST_SCHEMA
            && (!self.policies.require_valid_manifest || manifest_audit.valid)
            && !has_blocking(&issues, "ticket_bound_invalid")
            && !has_blocking(&issues, "parallelism_bound_invalid");
        let mut waves = Vec::new();
        if planning_started {
            let mut scheduled = BTreeMap::<String, usize>::new();
            let mut remaining = selected
                .keys()
                .filter(|id| {
                    self.policies.include_completed
                        || !matches!(selected[*id].status, TicketStatus::Done)
                })
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut wave_index = 0usize;
            loop {
                let mut chosen = Vec::new();
                let mut packages = BTreeSet::new();
                for id in &remaining {
                    if chosen.len() >= max_parallelism {
                        break;
                    }
                    let ticket = selected[id];
                    if matches!(ticket.status, TicketStatus::Blocked) {
                        continue;
                    }
                    if !missing_dependencies(ticket, &all_tickets).is_empty() {
                        continue;
                    }
                    let dependencies_scheduled = ticket.depends_on.iter().all(|dependency| {
                        all_tickets
                            .get(dependency.as_str())
                            .map(|candidate| matches!(candidate.status, TicketStatus::Done))
                            .unwrap_or(false)
                            || scheduled.contains_key(dependency)
                    });
                    if !dependencies_scheduled {
                        continue;
                    }
                    if self.policies.serialize_same_package
                        && !packages.insert(ticket.package.clone())
                    {
                        continue;
                    }
                    chosen.push(id.clone());
                }
                if chosen.is_empty() {
                    break;
                }
                let depends_on_waves = chosen
                    .iter()
                    .flat_map(|id| selected[id].depends_on.iter())
                    .filter_map(|dependency| scheduled.get(dependency).copied())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let package_ids = chosen
                    .iter()
                    .map(|id| selected[id].package.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                for id in &chosen {
                    scheduled.insert(id.clone(), wave_index);
                    remaining.remove(id);
                }
                waves.push(EngineeringPlanWave {
                    index: wave_index,
                    ticket_ids: chosen.clone(),
                    package_ids,
                    depends_on_waves,
                    parallelism: chosen.len(),
                });
                wave_index += 1;
            }
            for plan in &mut plans {
                if let Some(wave) = scheduled.get(&plan.ticket_id).copied() {
                    plan.scheduled = true;
                    plan.wave = Some(wave);
                }
            }
            for id in remaining {
                if plans
                    .iter()
                    .find(|plan| plan.ticket_id == id)
                    .is_some_and(|plan| plan.state == "ready")
                {
                    blocking(
                        &mut issues,
                        "ticket_not_scheduled",
                        id,
                        "ticket is dependency-ready but could not be placed in a bounded wave",
                        "increase the planning window/parallelism or inspect package serialization",
                    );
                }
            }
        }

        let critical_path = critical_path(&selected, &depths, &plans);
        let schedule_complete = plans
            .iter()
            .filter(|plan| {
                (self.policies.include_completed || plan.status != TicketStatus::Done)
                    && plan.state == "ready"
            })
            .all(|plan| plan.scheduled)
            || !planning_started;
        let dependency_closure = plans.iter().all(|plan| {
            plan.dependency_ids
                .iter()
                .all(|dependency| all_tickets.contains_key(dependency.as_str()))
        });
        let gates = vec![
            EngineeringPlanGate {
                name: "manifest_admission".into(),
                passed: manifest_audit.valid,
                required: self.policies.require_valid_manifest,
                detail: if manifest_audit.valid {
                    "engineering manifest has no blocking audit findings".into()
                } else {
                    "engineering manifest contains blocking findings".into()
                },
            },
            EngineeringPlanGate {
                name: "ticket_window".into(),
                passed: !truncated || self.policies.allow_truncation,
                required: !self.policies.allow_truncation,
                detail: if truncated {
                    format!(
                        "{omitted_ticket_count} tickets are outside the selected planning window"
                    )
                } else {
                    "all declared tickets are inside the planning window".into()
                },
            },
            EngineeringPlanGate {
                name: "dependency_closure".into(),
                passed: dependency_closure,
                required: true,
                detail: if dependency_closure {
                    "all selected ticket dependencies are declared".into()
                } else {
                    "one or more selected tickets name missing or unfinished dependencies".into()
                },
            },
            EngineeringPlanGate {
                name: "actionable_schedule".into(),
                passed: schedule_complete,
                required: planning_started,
                detail: if schedule_complete {
                    "every ready ticket is assigned to a bounded wave".into()
                } else {
                    "at least one ready ticket could not be assigned to a wave".into()
                },
            },
        ];
        let valid = self.schema == ENGINEERING_PLAN_REQUEST_SCHEMA
            && gates.iter().all(|gate| !gate.required || gate.passed)
            && !issues
                .iter()
                .any(|issue| issue.severity == IssueSeverity::Blocking);
        issues.sort_by(|left, right| {
            (&left.code, &left.subject, &left.detail).cmp(&(
                &right.code,
                &right.subject,
                &right.detail,
            ))
        });
        let plan_value = serde_json::to_value((
            &manifest_digest,
            &plans,
            &waves,
            &critical_path,
            &gates,
            &issues,
        ))
        .map_err(|error| EngineeringPlanError::Serialization(error.to_string()))?;
        let plan_digest = ContentHash::of_value(&plan_value)?.to_string();
        let planned_ticket_count = plans.iter().filter(|plan| plan.scheduled).count();
        Ok(EngineeringPlanAudit {
            schema: ENGINEERING_PLAN_AUDIT_SCHEMA.into(),
            request_schema: self.schema.clone(),
            manifest_digest,
            plan_digest,
            valid,
            planning_started,
            truncated,
            ticket_count,
            planned_ticket_count,
            omitted_ticket_count,
            package_order: manifest_audit.package_order,
            ticket_plans: plans,
            waves,
            critical_path,
            gates,
            manifest_issues: manifest_audit.issues,
            issues,
            guarantees: vec![
                "ticket waves are derived from declared dependencies and never from ticket-list order".into(),
                "same-package tickets can be serialized deterministically before parallel dispatch".into(),
                "critical path, readiness, blockers, and omitted-window counts remain separate".into(),
                "the plan digest binds the selected ticket plan and gate decisions".into(),
            ],
            limitations: vec![
                "the planner does not create or update tickets, run tests, execute CI, inspect a checkout, or contact a tracker".into(),
                "ticket status, package ownership, acceptance, and completion are caller-declared manifest evidence".into(),
                "a valid plan is an execution proposal, not proof that work was implemented or released".into(),
            ],
        })
    }
}

fn ticket_state(ticket: &TicketSpec, dependency_ready: bool) -> String {
    match ticket.status {
        TicketStatus::Done => "complete",
        TicketStatus::Blocked => "blocked",
        TicketStatus::Planned | TicketStatus::InProgress if !dependency_ready => "waiting",
        TicketStatus::Planned | TicketStatus::InProgress => "ready",
    }
    .into()
}

fn sorted_dependencies(ticket: &TicketSpec) -> Vec<String> {
    let mut dependencies = ticket.depends_on.clone();
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

fn blocking_dependencies(
    ticket: &TicketSpec,
    all_tickets: &BTreeMap<&str, &TicketSpec>,
) -> Vec<String> {
    let mut result = ticket
        .depends_on
        .iter()
        .filter(|dependency| {
            all_tickets
                .get(dependency.as_str())
                .map(|candidate| !matches!(candidate.status, TicketStatus::Done))
                .unwrap_or(true)
        })
        .cloned()
        .collect::<Vec<_>>();
    result.sort();
    result.dedup();
    result
}

fn missing_dependencies(
    ticket: &TicketSpec,
    all_tickets: &BTreeMap<&str, &TicketSpec>,
) -> Vec<String> {
    ticket
        .depends_on
        .iter()
        .filter(|dependency| !all_tickets.contains_key(dependency.as_str()))
        .cloned()
        .collect()
}

fn ticket_depth(
    id: &str,
    selected: &BTreeMap<String, &TicketSpec>,
    depths: &mut BTreeMap<String, usize>,
    visiting: &mut BTreeSet<String>,
) -> usize {
    if let Some(depth) = depths.get(id) {
        return *depth;
    }
    if !visiting.insert(id.to_string()) {
        return 0;
    }
    let depth = selected
        .get(id)
        .map(|ticket| {
            1 + ticket
                .depends_on
                .iter()
                .filter(|dependency| selected.contains_key(dependency.as_str()))
                .map(|dependency| ticket_depth(dependency, selected, depths, visiting))
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(1);
    visiting.remove(id);
    depths.insert(id.to_string(), depth);
    depth
}

fn critical_path(
    selected: &BTreeMap<String, &TicketSpec>,
    depths: &BTreeMap<String, usize>,
    plans: &[EngineeringTicketPlan],
) -> Vec<String> {
    let candidates = plans
        .iter()
        .filter(|plan| plan.status != TicketStatus::Done && plan.status != TicketStatus::Blocked)
        .map(|plan| plan.ticket_id.as_str())
        .collect::<Vec<_>>();
    let candidates = if candidates.is_empty() {
        selected.keys().map(String::as_str).collect::<Vec<_>>()
    } else {
        candidates
    };
    let Some(terminal) = candidates.into_iter().max_by(|left, right| {
        depths
            .get(*left)
            .unwrap_or(&0)
            .cmp(depths.get(*right).unwrap_or(&0))
            .then_with(|| right.cmp(left))
    }) else {
        return Vec::new();
    };
    let mut path = Vec::new();
    let mut current = terminal;
    let mut visited = BTreeSet::new();
    while visited.insert(current.to_string()) {
        path.push(current.to_string());
        let Some(ticket) = selected.get(current) else {
            break;
        };
        let Some(next) = ticket
            .depends_on
            .iter()
            .filter(|dependency| selected.contains_key(dependency.as_str()))
            .max_by(|left, right| {
                depths
                    .get(left.as_str())
                    .unwrap_or(&0)
                    .cmp(depths.get(right.as_str()).unwrap_or(&0))
                    .then_with(|| right.cmp(left))
            })
        else {
            break;
        };
        current = next;
    }
    path.reverse();
    path
}

fn default_true() -> bool {
    true
}

fn default_max_tickets() -> usize {
    MAX_PLAN_TICKETS
}

fn default_max_parallelism() -> usize {
    16
}

fn has_blocking(issues: &[EngineeringIssue], code: &str) -> bool {
    issues
        .iter()
        .any(|issue| issue.code == code && issue.severity == IssueSeverity::Blocking)
}

fn blocking(
    issues: &mut Vec<EngineeringIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(EngineeringIssue {
        code: code.into(),
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
        code: code.into(),
        severity: IssueSeverity::Warning,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engineering::{
        AdrSpec, AdrStatus, EngineeringPolicies, OwnershipSpec, PackageSpec, ProjectIdentity,
        TechnologyBaseline,
    };

    fn request() -> EngineeringPlanRequest {
        EngineeringPlanRequest {
            schema: ENGINEERING_PLAN_REQUEST_SCHEMA.into(),
            manifest: EngineeringManifest {
                schema: crate::engineering::ENGINEERING_MANIFEST_SCHEMA.into(),
                project: ProjectIdentity {
                    id: "aurora".into(),
                    version: "0.1.0".into(),
                    repository: "github.com/AURORA-NEURO/aurora-agent".into(),
                },
                baseline: TechnologyBaseline {
                    language: "Rust and Python".into(),
                    runtime: "cargo and CPython".into(),
                    api: "MCP".into(),
                    storage: "content addressed".into(),
                    observability: "structured traces".into(),
                    deployment: "reviewed".into(),
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
                        id: "T1".into(),
                        title: "build core".into(),
                        package: "core".into(),
                        contract: "core API".into(),
                        status: TicketStatus::Done,
                        depends_on: vec![],
                        acceptance: vec!["tests pass".into()],
                        blocker: None,
                    },
                    TicketSpec {
                        id: "T2".into(),
                        title: "build api".into(),
                        package: "api".into(),
                        contract: "api route".into(),
                        status: TicketStatus::Planned,
                        depends_on: vec!["T1".into()],
                        acceptance: vec!["protocol test passes".into()],
                        blocker: None,
                    },
                    TicketSpec {
                        id: "T3".into(),
                        title: "improve core".into(),
                        package: "core".into(),
                        contract: "core audit".into(),
                        status: TicketStatus::Planned,
                        depends_on: vec![],
                        acceptance: vec!["unit test passes".into()],
                        blocker: None,
                    },
                    TicketSpec {
                        id: "T4".into(),
                        title: "release api".into(),
                        package: "api".into(),
                        contract: "release gate".into(),
                        status: TicketStatus::Planned,
                        depends_on: vec!["T2".into()],
                        acceptance: vec!["release evidence exists".into()],
                        blocker: None,
                    },
                ],
                adrs: vec![AdrSpec {
                    id: "ADR-1".into(),
                    title: "split services".into(),
                    status: AdrStatus::Accepted,
                    decision: "keep API separate".into(),
                    affects: vec!["api".into()],
                    supersedes: None,
                }],
                ownership: vec![OwnershipSpec {
                    surface: "api".into(),
                    accountable: "platform-lead".into(),
                    responsible: vec!["api-team".into()],
                    consulted: vec![],
                    informed: vec![],
                    independent_reviewer: Some("review-board".into()),
                }],
                policies: EngineeringPolicies::default(),
            },
            policies: EngineeringPlanPolicies::default(),
        }
    }

    #[test]
    fn valid_manifest_derives_dependency_waves_and_critical_path() {
        let audit = request().audit().expect("plan");
        assert!(
            audit.valid,
            "issues: {:?}; manifest: {:?}",
            audit.issues, audit.manifest_issues
        );
        assert!(audit.planning_started);
        assert_eq!(audit.waves.len(), 2);
        assert_eq!(audit.waves[0].ticket_ids, vec!["T2", "T3"]);
        assert_eq!(audit.waves[1].ticket_ids, vec!["T4"]);
        assert_eq!(audit.critical_path, vec!["T1", "T2", "T4"]);
        assert_eq!(audit.planned_ticket_count, 3);
        assert!(audit
            .ticket_plans
            .iter()
            .find(|plan| plan.ticket_id == "T1")
            .unwrap()
            .wave
            .is_none());
    }

    #[test]
    fn invalid_manifest_stops_planning_and_preserves_manifest_findings() {
        let mut value = request();
        value.manifest.tickets[1].depends_on = vec!["missing".into()];
        let audit = value.audit().expect("plan");
        assert!(!audit.valid);
        assert!(!audit.planning_started);
        assert!(audit.waves.is_empty());
        assert!(audit
            .manifest_issues
            .iter()
            .any(|issue| issue.code == "missing_ticket_dependency"));
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "manifest_invalid"));
    }

    #[test]
    fn bounded_window_is_explicit_and_can_be_allowed_without_claiming_completeness() {
        let mut value = request();
        value.policies.max_tickets = 2;
        value.policies.allow_truncation = true;
        let audit = value.audit().expect("plan");
        assert!(audit.valid, "issues: {:?}", audit.issues);
        assert!(audit.truncated);
        assert_eq!(audit.omitted_ticket_count, 2);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "ticket_window_truncated"));
    }

    #[test]
    fn ticket_cycles_remain_blocking_even_when_invalid_manifest_planning_is_allowed() {
        let mut value = request();
        value.policies.require_valid_manifest = false;
        value.manifest.tickets[0].depends_on = vec!["T2".into()];
        value.manifest.tickets[1].depends_on = vec!["T1".into()];
        let audit = value.audit().expect("plan");
        assert!(!audit.valid);
        assert!(audit
            .issues
            .iter()
            .any(|issue| issue.code == "ticket_cycle"));
    }
}
