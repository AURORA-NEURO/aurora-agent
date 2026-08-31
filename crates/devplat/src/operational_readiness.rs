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
const MAX_TEXT_BYTES: usize = 4_096;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
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
    pub measurement_valid: bool,
    pub evidence_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalDependencyAudit {
    pub dependency_id: String,
    pub id_valid: bool,
    pub name_valid: bool,
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
    pub steps_valid: bool,
    pub step_count: usize,
    pub referenced_incidents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationalIncidentAudit {
    pub incident_id: String,
    pub valid: bool,
    pub owner_valid: bool,
    pub runbook_valid: bool,
    pub timeline_valid: bool,
    pub timeline_present: bool,
    pub postmortem_valid: bool,
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

        bound(
            &mut issues,
            "contracts",
            self.contracts.len(),
            MAX_CONTRACTS,
        );
        bound(
            &mut issues,
            "indicators",
            self.indicators.len(),
            MAX_INDICATORS,
        );
        bound(
            &mut issues,
            "dependencies",
            self.dependencies.len(),
            MAX_DEPENDENCIES,
        );
        bound(&mut issues, "runbooks", self.runbooks.len(), MAX_RUNBOOKS);
        bound(
            &mut issues,
            "incidents",
            self.incidents.len(),
            MAX_INCIDENTS,
        );
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
            if !valid_text(value) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    field,
                    format!(
                        "{field} must be non-empty, at most {MAX_TEXT_BYTES} bytes, and contain no control characters"
                    ),
                    "supply bounded visible metadata for the operational service",
                );
            }
        }

        for contract in &self.contracts {
            if !insert_unique(&mut contracts, &contract.id, "contract", &mut issues) {
                continue;
            }
            contracts.insert(contract.id.clone(), contract);
            if !valid_identifier(&contract.id)
                || !valid_text(&contract.objective)
                || !valid_text(&contract.target)
            {
                blocking(
                    &mut issues,
                    "contract_incomplete",
                    &contract.id,
                    "contract id, objective, and target must be canonical bounded visible text",
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
        if self.indicators.is_empty() && self.policies.require_observability {
            blocking(
                &mut issues,
                "indicators_missing",
                "indicators",
                "observability is required but the service declares no indicators",
                "declare at least one indicator for the service's operational objectives",
            );
        }

        for indicator in &self.indicators {
            if !insert_unique(&mut indicators, &indicator.id, "indicator", &mut issues) {
                continue;
            }
            indicators.insert(indicator.id.clone(), indicator);
            if !valid_identifier(&indicator.id)
                || !valid_identifier(&indicator.contract)
                || !valid_text(&indicator.metric)
                || !valid_text(&indicator.source)
            {
                blocking(
                    &mut issues,
                    "indicator_incomplete",
                    &indicator.id,
                    "indicator id, contract, metric, and source must be bounded visible text",
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
            if let Some(measurement) = indicator.measurement.as_deref() {
                if !valid_text(measurement) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("indicator.{}.measurement", indicator.id),
                        "indicator measurement contains invalid control or oversized text",
                        "supply bounded visible measurement text or omit it",
                    );
                }
            }
            if let Some(evidence_digest) = indicator.evidence_digest.as_deref() {
                if !valid_digest(evidence_digest) {
                    blocking(
                        &mut issues,
                        "indicator_evidence_noncanonical",
                        &indicator.id,
                        "indicator evidence digest is not a canonical lowercase content hash",
                        "store a lowercase 64-character content-addressed evidence digest",
                    );
                }
            }
            if indicator.status == IndicatorStatus::Observed
                && indicator
                    .measurement
                    .as_deref()
                    .is_none_or(|value| !valid_text(value))
            {
                blocking(
                    &mut issues,
                    "observed_indicator_measurement_missing",
                    &indicator.id,
                    "an observed indicator needs a non-empty measurement",
                    "retain the observed value alongside its content-addressed evidence record",
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
            if !valid_identifier(&dependency.id)
                || !valid_text(&dependency.name)
                || !valid_text(&dependency.owner)
                || !valid_text(&dependency.failure_mode)
            {
                blocking(
                    &mut issues,
                    "dependency_incomplete",
                    &dependency.id,
                    "dependency id, name, owner, and failure mode must be bounded visible text",
                    "declare who owns the dependency and how its failure appears",
                );
            }
            if let Some(fallback) = dependency.fallback.as_deref() {
                if !valid_text(fallback) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("dependency.{}.fallback", dependency.id),
                        "dependency fallback contains invalid control or oversized text",
                        "supply bounded visible degraded-mode text or omit it",
                    );
                }
            }
            if dependency.criticality == DependencyCriticality::Critical
                && self.policies.require_dependency_fallback
                && dependency
                    .fallback
                    .as_deref()
                    .is_none_or(|value| !valid_text(value))
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
            if !valid_identifier(&runbook.id)
                || !valid_text(&runbook.trigger)
                || !valid_text(&runbook.owner)
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
            if runbook.steps.iter().any(|step| step.trim().is_empty()) {
                blocking(
                    &mut issues,
                    "runbook_step_empty",
                    &runbook.id,
                    "runbook steps must contain executable non-empty instructions",
                    "replace empty steps with explicit operator actions",
                );
            }
            if runbook
                .steps
                .iter()
                .any(|step| !step.trim().is_empty() && !valid_text(step))
            {
                blocking(
                    &mut issues,
                    "runbook_step_invalid",
                    &runbook.id,
                    "runbook steps contain control characters or oversized text",
                    "keep each operator action bounded and visibly encoded",
                );
            }
            if runbook
                .incident_classes
                .iter()
                .any(|class| class.trim().is_empty())
            {
                blocking(
                    &mut issues,
                    "runbook_incident_class_empty",
                    &runbook.id,
                    "runbook incident classes must be non-empty when declared",
                    "remove empty classes or name the incident class explicitly",
                );
            }
            if runbook
                .incident_classes
                .iter()
                .any(|class| !class.trim().is_empty() && !valid_identifier(class))
            {
                blocking(
                    &mut issues,
                    "runbook_incident_class_invalid",
                    &runbook.id,
                    "runbook incident classes must be bounded visible identifiers",
                    "use stable incident-class identifiers without control characters",
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

        if self.runbooks.is_empty() && self.policies.require_runbooks {
            blocking(
                &mut issues,
                "runbooks_missing",
                "runbooks",
                "runbooks are required but the service declares none",
                "declare at least one reviewed runbook for the service's operational triggers",
            );
        }

        for incident in &self.incidents {
            if !insert_unique(&mut incidents, &incident.id, "incident", &mut issues) {
                continue;
            }
            incidents.insert(incident.id.clone(), incident);
            if incident.timeline.len() > MAX_LIST {
                bound(
                    &mut issues,
                    "incident.timeline",
                    incident.timeline.len(),
                    MAX_LIST,
                );
            }
            let runbook_valid = runbooks.contains_key(&incident.runbook);
            let timeline_present = !incident.timeline.is_empty();
            let timeline_valid =
                timeline_present && incident.timeline.iter().all(|entry| valid_text(entry));
            let closed = incident.state == IncidentState::Closed;
            let owner_valid = valid_text(&incident.owner);
            let postmortem_present = incident.postmortem.as_deref().is_some_and(valid_text);
            if !valid_identifier(&incident.id) || !valid_identifier(&incident.runbook) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    format!("incident.{}", incident.id),
                    "incident id and runbook reference must be canonical bounded identifiers",
                    "bind the incident to stable visible identifiers",
                );
            }
            if let Some(postmortem) = incident.postmortem.as_deref() {
                if !valid_text(postmortem) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("incident.{}.postmortem", incident.id),
                        "incident postmortem contains invalid control or oversized text",
                        "supply bounded visible learning text or omit it",
                    );
                }
            }
            if !owner_valid {
                blocking(
                    &mut issues,
                    "incident_owner_missing",
                    &incident.id,
                    "incident owner is empty",
                    "assign an accountable owner for incident follow-up",
                );
            }
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
            } else if !timeline_valid {
                blocking(
                    &mut issues,
                    "incident_timeline_entry_empty",
                    &incident.id,
                    "incident timeline contains an empty entry",
                    "retain a non-empty ordered response observation for every timeline entry",
                );
                if incident
                    .timeline
                    .iter()
                    .any(|entry| !entry.trim().is_empty() && !valid_text(entry))
                {
                    blocking(
                        &mut issues,
                        "incident_timeline_entry_invalid",
                        &incident.id,
                        "incident timeline contains control characters or oversized text",
                        "retain bounded visible response observations",
                    );
                }
            }
            if self.policies.require_incident_closure && !closed {
                blocking(
                    &mut issues,
                    "incident_not_closed",
                    &incident.id,
                    "an incident remains open or unresolved while closure is required",
                    "resolve the incident and retain its closure learning record",
                );
            }
            if self.policies.require_incident_closure && closed && !postmortem_present {
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
            (
                "alerting",
                self.controls.alerting,
                self.policies.require_observability,
            ),
            (
                "tracing",
                self.controls.tracing,
                self.policies.require_observability,
            ),
            (
                "audit_logging",
                self.controls.audit_logging,
                self.policies.require_observability,
            ),
            (
                "backup",
                self.controls.backup,
                self.service.criticality != OperationalCriticality::Advisory,
            ),
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
                let source_valid = valid_text(&indicator.source);
                let observed = indicator.status == IndicatorStatus::Observed;
                let measurement_valid =
                    !observed || indicator.measurement.as_deref().is_some_and(valid_text);
                let evidence_valid = !observed
                    || indicator.evidence_digest.as_deref().map(valid_digest) == Some(true);
                OperationalIndicatorAudit {
                    indicator_id: indicator.id.clone(),
                    contract_valid,
                    source_valid,
                    observed,
                    measurement_valid,
                    evidence_valid,
                    ready: contract_valid
                        && source_valid
                        && measurement_valid
                        && evidence_valid
                        && (observed || indicator.status == IndicatorStatus::NotApplicable),
                }
            })
            .collect::<Vec<_>>();
        let dependency_audits = self
            .dependencies
            .iter()
            .map(|dependency| {
                let id_valid = valid_identifier(&dependency.id);
                let name_valid = valid_text(&dependency.name);
                let owner_valid = valid_text(&dependency.owner);
                let failure_mode_valid = valid_text(&dependency.failure_mode);
                let fallback_present = dependency.fallback.as_deref().is_some_and(valid_text);
                let critical = dependency.criticality == DependencyCriticality::Critical;
                OperationalDependencyAudit {
                    dependency_id: dependency.id.clone(),
                    id_valid,
                    name_valid,
                    owner_valid,
                    failure_mode_valid,
                    fallback_present,
                    critical,
                    ready: id_valid
                        && name_valid
                        && owner_valid
                        && failure_mode_valid
                        && (!critical
                            || !self.policies.require_dependency_fallback
                            || fallback_present),
                }
            })
            .collect::<Vec<_>>();
        let runbook_audits = self
            .runbooks
            .iter()
            .map(|runbook| OperationalRunbookAudit {
                runbook_id: runbook.id.clone(),
                valid: valid_identifier(&runbook.id)
                    && valid_text(&runbook.trigger)
                    && valid_text(&runbook.owner)
                    && !runbook.steps.is_empty()
                    && runbook.steps.iter().all(|step| valid_text(step))
                    && runbook
                        .incident_classes
                        .iter()
                        .all(|class| valid_identifier(class)),
                review_current: runbook.review_status == RunbookReviewStatus::Reviewed,
                steps_valid: !runbook.steps.is_empty()
                    && runbook.steps.iter().all(|step| valid_text(step)),
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
                valid: valid_identifier(&incident.id)
                    && valid_identifier(&incident.runbook)
                    && valid_text(&incident.owner)
                    && runbooks.contains_key(&incident.runbook)
                    && !incident.timeline.is_empty()
                    && incident.timeline.iter().all(|entry| valid_text(entry))
                    && (incident.state != IncidentState::Closed
                        || incident.postmortem.as_deref().is_some_and(valid_text)),
                owner_valid: valid_text(&incident.owner),
                runbook_valid: runbooks.contains_key(&incident.runbook),
                timeline_valid: !incident.timeline.is_empty()
                    && incident.timeline.iter().all(|entry| valid_text(entry)),
                timeline_present: !incident.timeline.is_empty(),
                postmortem_valid: incident.state != IncidentState::Closed
                    || incident.postmortem.as_deref().is_some_and(valid_text),
                postmortem_present: incident.postmortem.as_deref().is_some_and(valid_text),
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
            enabled_controls: control_rows
                .iter()
                .filter(|(_, enabled, _)| *enabled)
                .count(),
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

fn insert_unique<T>(
    map: &mut BTreeMap<String, &T>,
    id: &str,
    kind: &'static str,
    issues: &mut Vec<OperationalReadinessIssue>,
) -> bool {
    if map
        .keys()
        .any(|existing| existing == id || existing.eq_ignore_ascii_case(id))
    {
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

fn bound(issues: &mut Vec<OperationalReadinessIssue>, subject: &str, count: usize, maximum: usize) {
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
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn valid_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && value == trimmed
        && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value) && value == value.trim()
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
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "indicator_not_observed"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "critical_dependency_fallback_missing"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "required_control_disabled"));
    }

    #[test]
    fn closed_incident_without_learning_record_is_not_closed_readiness() {
        let mut value = manifest();
        value.incidents[0].postmortem = None;
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "closed_incident_postmortem_missing"));
    }

    #[test]
    fn observed_indicator_requires_measurement_as_well_as_evidence() {
        let mut value = manifest();
        value.indicators[0].measurement = None;
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "observed_indicator_measurement_missing"));
        assert!(!report.indicator_audits[0].measurement_valid);
        assert!(!report.indicator_audits[0].ready);
    }

    #[test]
    fn required_observability_and_runbooks_cannot_be_satisfied_by_empty_sections() {
        let mut value = manifest();
        value.indicators.clear();
        value.runbooks.clear();
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "indicators_missing"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "runbooks_missing"));
    }

    #[test]
    fn incident_and_runbook_rows_require_non_empty_operational_detail() {
        let mut value = manifest();
        value.runbooks[0].steps[0] = "  ".into();
        value.incidents[0].state = IncidentState::Open;
        value.incidents[0].owner = "".into();
        value.incidents[0].timeline[1] = " ".into();
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "runbook_step_empty"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "incident_owner_missing"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "incident_timeline_entry_empty"));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "incident_not_closed"));
        assert!(!report.incident_audits[0].valid);
        assert!(!report.incident_audits[0].timeline_valid);
    }

    #[test]
    fn dependency_audit_does_not_report_a_malformed_name_as_ready() {
        let mut value = manifest();
        value.dependencies[0].name = " ".into();
        let report = value.audit().unwrap();
        assert!(!report.valid);
        assert!(!report.dependency_audits[0].name_valid);
        assert!(!report.dependency_audits[0].ready);
    }

    #[test]
    fn operational_readiness_rejects_noncanonical_evidence_and_control_metadata() {
        let mut value = manifest();
        value.service.owner = "platform\noncall".into();
        value.indicators[0].evidence_digest = Some("A".repeat(64));
        value.indicators[0].measurement = Some("0.999\u{0000}".into());
        value.dependencies[0].fallback = Some("offline\u{0007}mirror".into());
        value.incidents[0].timeline[1] = "contained\u{000b}".into();

        let report = value.audit().expect("audit");
        assert!(!report.valid);
        for code in [
            "field_invalid",
            "indicator_evidence_noncanonical",
            "incident_timeline_entry_invalid",
        ] {
            assert!(
                report.issues.iter().any(|issue| issue.code == code),
                "missing {code}"
            );
        }
        assert!(!valid_digest(&"A".repeat(64)));
        assert!(valid_digest(&"a".repeat(64)));
    }

    #[test]
    fn operational_readiness_rejects_case_colliding_contracts_and_bounds_timelines() {
        let mut value = manifest();
        let mut duplicate = value.contracts[0].clone();
        duplicate.id = "AVAILABILITY".into();
        value.contracts.push(duplicate);
        value.incidents[0].timeline = vec!["observed".into(); MAX_LIST + 1];

        let report = value.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "duplicate_contract_id"));
        assert!(report.issues.iter().any(|issue| {
            issue.code == "input_bound_exceeded" && issue.subject == "incident.timeline"
        }));
    }

    #[test]
    fn operational_readiness_rejects_padded_measurement_text() {
        let mut value = manifest();
        value.contracts[0].objective = " availability".into();
        let report = value.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "contract_incomplete"));
    }
}
