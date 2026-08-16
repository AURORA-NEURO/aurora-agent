//! Deterministic audit of a service's operational-readiness declaration.
//!
//! This is the operational companion to the engineering and release-pipeline manifests. It keeps
//! objectives, observed indicators, dependency fallbacks, runbooks, incident closure, and control
//! posture in one bounded artifact without pretending that a declaration is a live telemetry
//! query, an incident-management system, an on-call schedule, or a deployment authorization.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const OPERATIONAL_READINESS_MANIFEST_SCHEMA: &str = "bioprism-operational-readiness/0.1";
pub const OPERATIONAL_READINESS_AUDIT_SCHEMA: &str = "bioprism-operational-readiness-audit/0.1";

const MAX_CONTRACTS: usize = 4_096;
const MAX_INDICATORS: usize = 8_192;
const MAX_DEPENDENCIES: usize = 8_192;
const MAX_RUNBOOKS: usize = 4_096;
const MAX_INCIDENTS: usize = 4_096;
const MAX_CONTROLS: usize = 256;
const MAX_LIST: usize = 16_384;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalReadinessManifest {
    pub schema: String,
    pub service: OperationalService,
    #[serde(default)]
    pub contracts: Vec<OperationalContract>,
    #[serde(default)]
    pub indicators: Vec<OperationalIndicator>,
    #[serde(default)]
    pub dependencies: Vec<OperationalDependency>,
    #[serde(default)]
    pub runbooks: Vec<OperationalRunbook>,
    #[serde(default)]
    pub incidents: Vec<OperationalIncident>,
    #[serde(default)]
    pub controls: OperationalControls,
    #[serde(default)]
    pub policies: OperationalReadinessPolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalService {
    pub id: String,
    pub version: String,
    pub owner: String,
    pub criticality: OperationalCriticality,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum OperationalCriticality {
    Critical,
    Important,
    Advisory,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum OperationalContractKind {
    Availability,
    Latency,
    Durability,
    Recovery,
    Security,
    Privacy,
    Capacity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalContract {
    pub id: String,
    pub kind: OperationalContractKind,
    pub objective: String,
    pub target: String,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorStatus {
    Observed,
    NotObserved,
    Blocked,
    NotApplicable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalIndicator {
    pub id: String,
    pub contract: String,
    pub metric: String,
    pub source: String,
    pub status: IndicatorStatus,
    #[serde(default)]
    pub measurement: Option<String>,
    #[serde(default)]
    pub evidence_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DependencyCriticality {
    Critical,
    Important,
    Advisory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalDependency {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub criticality: DependencyCriticality,
    pub failure_mode: String,
    #[serde(default)]
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RunbookReviewStatus {
    Draft,
    Reviewed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalRunbook {
    pub id: String,
    pub trigger: String,
    pub owner: String,
    pub steps: Vec<String>,
    pub review_status: RunbookReviewStatus,
    #[serde(default)]
    pub incident_classes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IncidentSeverity {
    Sev1,
    Sev2,
    Sev3,
    Sev4,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IncidentState {
    Open,
    Contained,
    Resolved,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalIncident {
    pub id: String,
    pub severity: IncidentSeverity,
    pub state: IncidentState,
    pub runbook: String,
    pub owner: String,
    #[serde(default)]
    pub timeline: Vec<String>,
    #[serde(default)]
    pub postmortem: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalControls {
    #[serde(default)]
    pub on_call: bool,
    #[serde(default)]
    pub alerting: bool,
    #[serde(default)]
    pub tracing: bool,
    #[serde(default)]
    pub audit_logging: bool,
    #[serde(default)]
    pub backup: bool,
    #[serde(default)]
    pub restore_test: bool,
    #[serde(default)]
    pub access_review: bool,
}

impl Default for OperationalControls {
    fn default() -> Self {
        Self {
            on_call: false,
            alerting: false,
            tracing: false,
            audit_logging: false,
            backup: false,
            restore_test: false,
            access_review: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalReadinessPolicies {
    #[serde(default = "default_true")]
    pub require_contract_evidence: bool,
    #[serde(default = "default_true")]
    pub require_observability: bool,
    #[serde(default = "default_true")]
    pub require_runbooks: bool,
    #[serde(default = "default_true")]
    pub require_restore_test: bool,
    #[serde(default = "default_true")]
    pub require_dependency_fallback: bool,
    #[serde(default = "default_true")]
    pub require_incident_closure: bool,
    #[serde(default = "default_true")]
    pub require_access_review: bool,
}

impl Default for OperationalReadinessPolicies {
    fn default() -> Self {
        Self {
            require_contract_evidence: true,
            require_observability: true,
            require_runbooks: true,
            require_restore_test: true,
            require_dependency_fallback: true,
            require_incident_closure: true,
            require_access_review: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationalIssueSeverity {
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalReadinessIssue {
    pub code: String,
    pub severity: OperationalIssueSeverity,
    pub subject: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalIndicatorAudit {
    pub indicator_id: String,
    pub contract_valid: bool,
    pub source_valid: bool,
    pub observed: bool,
    pub evidence_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalDependencyAudit {
    pub dependency_id: String,
    pub owner_valid: bool,
    pub failure_mode_valid: bool,
    pub fallback_present: bool,
    pub critical: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalRunbookAudit {
    pub runbook_id: String,
    pub valid: bool,
    pub review_current: bool,
    pub step_count: usize,
    pub referenced_incidents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalIncidentAudit {
    pub incident_id: String,
    pub valid: bool,
    pub runbook_valid: bool,
    pub timeline_present: bool,
    pub postmortem_present: bool,
    pub closed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalControlAudit {
    pub control: String,
    pub enabled: bool,
    pub required: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalReadinessCounts {
    pub contracts: usize,
    pub required_contracts: usize,
    pub indicators: usize,
    pub observed_indicators: usize,
    pub dependencies: usize,
    pub critical_dependencies: usize,
    pub runbooks: usize,
    pub incidents: usize,
    pub open_incidents: usize,
    pub controls: usize,
    pub enabled_controls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalReadinessAudit {
    pub schema: String,
    pub manifest_schema: String,
    pub digest: String,
    pub valid: bool,
    pub service_id: String,
    pub counts: OperationalReadinessCounts,
    pub indicator_audits: Vec<OperationalIndicatorAudit>,
    pub dependency_audits: Vec<OperationalDependencyAudit>,
    pub runbook_audits: Vec<OperationalRunbookAudit>,
    pub incident_audits: Vec<OperationalIncidentAudit>,
    pub control_audits: Vec<OperationalControlAudit>,
    pub issues: Vec<OperationalReadinessIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum OperationalReadinessError {
    #[error("cannot canonicalize operational-readiness manifest: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize operational-readiness manifest: {0}")]
    Serialization(String),
}

impl OperationalReadinessManifest {
    pub fn digest(&self) -> Result<ContentHash, OperationalReadinessError> {
        let value = serde_json::to_value(self)
            .map_err(|error| OperationalReadinessError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<OperationalReadinessAudit, OperationalReadinessError> {
        let digest = self.digest()?.to_string();
        let mut issues = Vec::new();
        let mut contracts = BTreeMap::<String, &OperationalContract>::new();
        let mut indicators = BTreeMap::<String, &OperationalIndicator>::new();
        let mut dependencies = BTreeMap::<String, &OperationalDependency>::new();
        let mut runbooks = BTreeMap::<String, &OperationalRunbook>::new();
        let mut incidents = BTreeMap::<String, &OperationalIncident>::new();

        bound(&mut issues, "contracts", self.contracts.len(), MAX_CONTRACTS);
        bound(&mut issues, "indicators", self.indicators.len(), MAX_INDICATORS);
        bound(&mut issues, "dependencies", self.dependencies.len(), MAX_DEPENDENCIES);
        bound(&mut issues, "runbooks", self.runbooks.len(), MAX_RUNBOOKS);
        bound(&mut issues, "incidents", self.incidents.len(), MAX_INCIDENTS);
        if self.schema != OPERATIONAL_READINESS_MANIFEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "manifest",
                format!(
                    "expected {OPERATIONAL_READINESS_MANIFEST_SCHEMA}, got {}",
                    self.schema
                ),
                "regenerate the declaration with the published operational-readiness schema",
            );
        }
        for (field, value) in [
            ("service.id", &self.service.id),
            ("service.version", &self.service.version),
            ("service.owner", &self.service.owner),
        ] {
            if value.trim().is_empty() {
                blocking(
                    &mut issues,
                    "required_field_empty",
                    field,
                    format!("{field} is empty"),
                    "declare the service identity and accountable owner",
                );
            }
        }

        for contract in &self.contracts {
            if !insert_unique(&mut contracts, &contract.id, "contract", &mut issues) {
                continue;
            }
            contracts.insert(contract.id.clone(), contract);
            if contract.id.trim().is_empty()
                || contract.objective.trim().is_empty()
                || contract.target.trim().is_empty()
            {
                blocking(
                    &mut issues,
                    "contract_incomplete",
                    &contract.id,
                    "contract id, objective, and target are required",
                    "state a measurable operational objective and target",
                );
            }
        }
        if self.contracts.is_empty() && self.policies.require_contract_evidence {
            blocking(
                &mut issues,
                "contracts_missing",
                "contracts",
                "the service declares no operational objectives",
                "declare at least one objective with a target and an indicator",
            );
        }

        for indicator in &self.indicators {
            if !insert_unique(&mut indicators, &indicator.id, "indicator", &mut issues) {
                continue;
            }
            indicators.insert(indicator.id.clone(), indicator);
            if indicator.id.trim().is_empty()
                || indicator.metric.trim().is_empty()
                || indicator.source.trim().is_empty()
            {
                blocking(
                    &mut issues,
                    "indicator_incomplete",
                    &indicator.id,
                    "indicator id, metric, and source are required",
                    "bind the indicator to a named measurement source",
                );
            }
            if !contracts.contains_key(&indicator.contract) {
                blocking(
                    &mut issues,
                    "indicator_contract_missing",
                    &indicator.id,
                    format!("indicator names undeclared contract {}", indicator.contract),
                    "declare the contract before attaching an indicator",
                );
            }
            if indicator.status == IndicatorStatus::Observed
                && indicator.evidence_digest.as_deref().map(valid_digest) != Some(true)
            {
                blocking(
                    &mut issues,
                    "observed_indicator_evidence_missing",
                    &indicator.id,
                    "an observed indicator needs a 64-character evidence digest",
                    "bind observed telemetry to a content-addressed evidence record",
                );
            }
            if self.policies.require_observability
                && indicator.status != IndicatorStatus::Observed
                && indicator.status != IndicatorStatus::NotApplicable
            {
                blocking(
                    &mut issues,
                    "indicator_not_observed",
                    &indicator.id,
                    "required indicator is not observed",
                    "collect the indicator or explicitly justify why it is not applicable",
                );
            }
        }
        for contract in &self.contracts {
            if contract.required
                && self.policies.require_contract_evidence
                && !self
                    .indicators
                    .iter()
                    .any(|indicator| indicator.contract == contract.id)
            {
                blocking(
                    &mut issues,
                    "contract_indicator_missing",
                    &contract.id,
                    "required operational contract has no indicator",
                    "attach at least one indicator to every required contract",
                );
            }
        }

        for dependency in &self.dependencies {
            if !insert_unique(&mut dependencies, &dependency.id, "dependency", &mut issues) {
                continue;
            }
            dependencies.insert(dependency.id.clone(), dependency);
            if dependency.id.trim().is_empty()
                || dependency.name.trim().is_empty()
                || dependency.owner.trim().is_empty()
                || dependency.failure_mode.trim().is_empty()
            {
                blocking(
                    &mut issues,
                    "dependency_incomplete",
                    &dependency.id,
                    "dependency id, name, owner, and failure mode are required",
                    "declare who owns the dependency and how its failure appears",
                );
            }
            if dependency.criticality == DependencyCriticality::Critical
                && self.policies.require_dependency_fallback
                && dependency
                    .fallback
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
            {
                blocking(
                    &mut issues,
                    "critical_dependency_fallback_missing",
                    &dependency.id,
                    "critical dependency has no fallback or degraded mode",
                    "declare a tested fallback, isolation, or explicit degraded behavior",
                );
            }
        }

        for runbook in &self.runbooks {
            if !insert_unique(&mut runbooks, &runbook.id, "runbook", &mut issues) {
                continue;
            }
            runbooks.insert(runbook.id.clone(), runbook);
            if runbook.id.trim().is_empty()
                || runbook.trigger.trim().is_empty()
                || runbook.owner.trim().is_empty()
                || runbook.steps.is_empty()
            {
                blocking(
                    &mut issues,
                    "runbook_incomplete",
                    &runbook.id,
                    "runbook id, trigger, owner, and at least one step are required",
                    "provide an owned, executable response sequence",
                );
            }
            if runbook.steps.len() > MAX_LIST || runbook.incident_classes.len() > MAX_LIST {
                bound(
                    &mut issues,
                    "runbook.list",
                    runbook.steps.len().max(runbook.incident_classes.len()),
                    MAX_LIST,
                );
            }
            if self.policies.require_runbooks
                && runbook.review_status != RunbookReviewStatus::Reviewed
            {
                blocking(
                    &mut issues,
                    "runbook_not_current",
                    &runbook.id,
                    "required runbook is draft or expired",
                    "review the runbook against the current service and dependency topology",
                );
            }
        }

        for incident in &self.incidents {
            if !insert_unique(&mut incidents, &incident.id, "incident", &mut issues) {
                continue;
            }
            incidents.insert(incident.id.clone(), incident);
            let runbook_valid = runbooks.contains_key(&incident.runbook);
            let timeline_present = !incident.timeline.is_empty();
            let closed = incident.state == IncidentState::Closed;
            let postmortem_present = incident
                .postmortem
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty());
            if !runbook_valid {
                blocking(
                    &mut issues,
                    "incident_runbook_missing",
                    &incident.id,
                    format!("incident names undeclared runbook {}", incident.runbook),
                    "bind each incident to the runbook that governed its response",
                );
            }
            if !timeline_present {
                blocking(
                    &mut issues,
                    "incident_timeline_missing",
                    &incident.id,
                    "incident has no timeline entries",
                    "retain ordered response observations instead of only a terminal label",
                );
            }
            if self.policies.require_incident_closure
                && closed
                && !postmortem_present
            {
                blocking(
                    &mut issues,
                    "closed_incident_postmortem_missing",
                    &incident.id,
                    "closed incident has no postmortem or learning record",
                    "attach a postmortem before declaring the incident closed",
                );
            }
        }

        let control_rows = [
            ("on_call", self.controls.on_call, true),
            ("alerting", self.controls.alerting, self.policies.require_observability),
            ("tracing", self.controls.tracing, self.policies.require_observability),
            (
                "audit_logging",
                self.controls.audit_logging,
                self.policies.require_observability,
            ),
            ("backup", self.controls.backup, self.service.criticality != OperationalCriticality::Advisory),
            (
                "restore_test",
                self.controls.restore_test,
                self.policies.require_restore_test,
            ),
            (
                "access_review",
                self.controls.access_review,
                self.policies.require_access_review,
            ),
        ];
        if control_rows.len() > MAX_CONTROLS {
            bound(&mut issues, "controls", control_rows.len(), MAX_CONTROLS);
        }
        for (name, enabled, required) in control_rows {
            if required && !enabled {
                blocking(
                    &mut issues,
                    "required_control_disabled",
                    name,
                    format!("required operational control {name} is disabled"),
                    "enable the control or turn off the policy only with an explicit risk decision",
                );
            }
        }

        let indicator_audits = self
            .indicators
            .iter()
            .map(|indicator| {
                let contract_valid = contracts.contains_key(&indicator.contract);
                let source_valid = !indicator.source.trim().is_empty();
                let observed = indicator.status == IndicatorStatus::Observed;
                let evidence_valid = !observed
                    || indicator.evidence_digest.as_deref().map(valid_digest) == Some(true);
                OperationalIndicatorAudit {
                    indicator_id: indicator.id.clone(),
                    contract_valid,
                    source_valid,
                    observed,
                    evidence_valid,
                    ready: contract_valid && source_valid && evidence_valid && (observed || indicator.status == IndicatorStatus::NotApplicable),
                }
            })
            .collect::<Vec<_>>();
        let dependency_audits = self
            .dependencies
            .iter()
            .map(|dependency| {
                let owner_valid = !dependency.owner.trim().is_empty();
                let failure_mode_valid = !dependency.failure_mode.trim().is_empty();
                let fallback_present = dependency
                    .fallback
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty());
                let critical = dependency.criticality == DependencyCriticality::Critical;
                OperationalDependencyAudit {
                    dependency_id: dependency.id.clone(),
                    owner_valid,
                    failure_mode_valid,
                    fallback_present,
                    critical,
                    ready: owner_valid
                        && failure_mode_valid
                        && (!critical || !self.policies.require_dependency_fallback || fallback_present),
                }
            })
            .collect::<Vec<_>>();
        let runbook_audits = self
            .runbooks
            .iter()
            .map(|runbook| OperationalRunbookAudit {
                runbook_id: runbook.id.clone(),
                valid: !runbook.id.trim().is_empty()
                    && !runbook.trigger.trim().is_empty()
                    && !runbook.owner.trim().is_empty()
                    && !runbook.steps.is_empty(),
                review_current: runbook.review_status == RunbookReviewStatus::Reviewed,
                step_count: runbook.steps.len(),
                referenced_incidents: self
                    .incidents
                    .iter()
                    .filter(|incident| incident.runbook == runbook.id)
                    .count(),
            })
            .collect::<Vec<_>>();
        let incident_audits = self
            .incidents
            .iter()
            .map(|incident| OperationalIncidentAudit {
                incident_id: incident.id.clone(),
                valid: runbooks.contains_key(&incident.runbook) && !incident.timeline.is_empty(),
                runbook_valid: runbooks.contains_key(&incident.runbook),
                timeline_present: !incident.timeline.is_empty(),
                postmortem_present: incident
                    .postmortem
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|value| !value.is_empty()),
                closed: incident.state == IncidentState::Closed,
            })
            .collect::<Vec<_>>();
        let control_audits = control_rows
            .iter()
            .map(|(control, enabled, required)| OperationalControlAudit {
                control: (*control).into(),
                enabled: *enabled,
                required: *required,
                ready: !*required || *enabled,
            })
            .collect::<Vec<_>>();

        let counts = OperationalReadinessCounts {
            contracts: self.contracts.len(),
            required_contracts: self.contracts.iter().filter(|item| item.required).count(),
            indicators: self.indicators.len(),
            observed_indicators: self
                .indicators
                .iter()
                .filter(|item| item.status == IndicatorStatus::Observed)
                .count(),
            dependencies: self.dependencies.len(),
            critical_dependencies: self
                .dependencies
                .iter()
                .filter(|item| item.criticality == DependencyCriticality::Critical)
                .count(),
            runbooks: self.runbooks.len(),
            incidents: self.incidents.len(),
            open_incidents: self
                .incidents
                .iter()
                .filter(|item| item.state == IncidentState::Open)
                .count(),
            controls: control_rows.len(),
            enabled_controls: control_rows.iter().filter(|(_, enabled, _)| *enabled).count(),
        };
        let valid = !issues
            .iter()
            .any(|issue| issue.severity == OperationalIssueSeverity::Blocking);
        Ok(OperationalReadinessAudit {
            schema: OPERATIONAL_READINESS_AUDIT_SCHEMA.into(),
            manifest_schema: self.schema.clone(),
            digest,
            valid,
            service_id: self.service.id.clone(),
            counts,
            indicator_audits,
            dependency_audits,
            runbook_audits,
            incident_audits,
            control_audits,
            issues,
            guarantees: vec![
                "service objectives, indicators, dependencies, runbooks, incidents, and controls are audited as separate layers".into(),
                "observed indicators are bound to caller-declared evidence digests rather than treated as unqualified numbers".into(),
                "critical dependencies and closed incidents retain explicit fallback and learning obligations".into(),
            ],
            limitations: vec![
                "the audit does not query telemetry, page an on-call team, inspect a live dependency, or open an incident".into(),
                "evidence digests, review status, owners, and control booleans are caller-declared".into(),
                "a valid declaration is operational readiness evidence, not proof of uptime, recovery, or safe production behavior".into(),
            ],
        })
    }
}

fn insert_unique<'a, T>(
    map: &mut BTreeMap<String, &'a T>,
    id: &str,
    kind: &'static str,
    issues: &mut Vec<OperationalReadinessIssue>,
) -> bool {
    if map.contains_key(id) {
        blocking(
            issues,
            &format!("duplicate_{kind}_id"),
            id,
            format!("{kind} identifier occurs more than once"),
            format!("assign one stable identifier to exactly one {kind}"),
        );
        false
    } else {
        true
    }
}

fn bound(
    issues: &mut Vec<OperationalReadinessIssue>,
    subject: &str,
    count: usize,
    maximum: usize,
) {
    if count > maximum {
        blocking(
            issues,
            "input_bound_exceeded",
            subject,
            format!("{count} entries exceed maximum {maximum}"),
            "split the manifest or reduce the declared surface",
        );
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn blocking(
    issues: &mut Vec<OperationalReadinessIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(OperationalReadinessIssue {
        code: code.into(),
        severity: OperationalIssueSeverity::Blocking,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> OperationalReadinessManifest {
        OperationalReadinessManifest {
            schema: OPERATIONAL_READINESS_MANIFEST_SCHEMA.into(),
            service: OperationalService {
                id: "prism-api".into(),
                version: "0.1.0".into(),
                owner: "platform-oncall".into(),
                criticality: OperationalCriticality::Critical,
            },
            contracts: vec![
                OperationalContract {
                    id: "availability".into(),
                    kind: OperationalContractKind::Availability,
                    objective: "serve health checks".into(),
                    target: "99.9%".into(),
                    required: true,
                },
                OperationalContract {
                    id: "recovery".into(),
                    kind: OperationalContractKind::Recovery,
                    objective: "restore service".into(),
                    target: "RTO <= 1h".into(),
                    required: true,
                },
            ],
            indicators: vec![
                OperationalIndicator {
                    id: "availability-sli".into(),
                    contract: "availability".into(),
                    metric: "request_success_ratio".into(),
                    source: "telemetry-digest".into(),
                    status: IndicatorStatus::Observed,
                    measurement: Some("0.999".into()),
                    evidence_digest: Some("a".repeat(64)),
                },
                OperationalIndicator {
                    id: "recovery-sli".into(),
                    contract: "recovery".into(),
                    metric: "restore_duration".into(),
                    source: "restore-test-2026-08".into(),
                    status: IndicatorStatus::Observed,
                    measurement: Some("42m".into()),
                    evidence_digest: Some("b".repeat(64)),
                },
            ],
            dependencies: vec![OperationalDependency {
                id: "registry".into(),
                name: "artifact registry".into(),
                owner: "release-team".into(),
                criticality: DependencyCriticality::Critical,
                failure_mode: "artifact fetch unavailable".into(),
                fallback: Some("pinned offline mirror".into()),
            }],
            runbooks: vec![OperationalRunbook {
                id: "api-degraded".into(),
                trigger: "availability below target".into(),
                owner: "platform-oncall".into(),
                steps: vec!["freeze rollout".into(), "restore last known good".into()],
                review_status: RunbookReviewStatus::Reviewed,
                incident_classes: vec!["availability".into()],
            }],
            incidents: vec![OperationalIncident {
                id: "inc-1".into(),
                severity: IncidentSeverity::Sev2,
                state: IncidentState::Closed,
                runbook: "api-degraded".into(),
                owner: "platform-oncall".into(),
                timeline: vec!["detected".into(), "contained".into(), "restored".into()],
                postmortem: Some("postmortem-digest".into()),
            }],
            controls: OperationalControls {
                on_call: true,
                alerting: true,
                tracing: true,
                audit_logging: true,
                backup: true,
                restore_test: true,
                access_review: true,
            },
            policies: OperationalReadinessPolicies::default(),
        }
    }

    #[test]
    fn valid_operational_manifest_preserves_each_readiness_layer() {
        let report = manifest().audit().unwrap();
        assert!(report.valid);
        assert_eq!(report.counts.observed_indicators, 2);
        assert_eq!(report.counts.critical_dependencies, 1);
        assert!(report.indicator_audits.iter().all(|item| item.ready));
        assert!(report.dependency_audits[0].fallback_present);
        assert!(report.incident_audits[0].postmortem_present);
    }

    #[test]
    fn missing_observation_fallback_and_control_are_blocking() {
        let mut value = manifest();
        value.indicators[0].status = IndicatorStatus::NotObserved;
        value.dependencies[0].fallback = None;
        value.controls.restore_test = false;
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "indicator_not_observed"));
        assert!(report.issues.iter().any(|issue| issue.code == "critical_dependency_fallback_missing"));
        assert!(report.issues.iter().any(|issue| issue.code == "required_control_disabled"));
    }

    #[test]
    fn closed_incident_without_learning_record_is_not_closed_readiness() {
        let mut value = manifest();
        value.incidents[0].postmortem = None;
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report.issues.iter().any(|issue| issue.code == "closed_incident_postmortem_missing"));
    }
}
