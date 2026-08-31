//! Deterministic governance evidence for a security, safety, and red-team program.
//!
//! The existing `security_redteam_simulate` route models bounded red-team and incident
//! transitions. This kernel audits the program declaration around that simulator: authorized
//! scope, independent campaign review, evidence, findings, remediation, incident response,
//! disclosure sequencing, and regression controls remain separate rows. It never runs a fuzzer,
//! probes a target, contains a service, sends a disclosure, or claims a live control exists.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const SECURITY_PROGRAM_MANIFEST_SCHEMA: &str = "bioprism-security-program/0.1";
pub const SECURITY_PROGRAM_AUDIT_SCHEMA: &str = "bioprism-security-program-audit/0.1";

const MAX_SCOPES: usize = 4_096;
const MAX_CAMPAIGNS: usize = 8_192;
const MAX_FINDINGS: usize = 16_384;
const MAX_REMEDIATIONS: usize = 16_384;
const MAX_INCIDENTS: usize = 8_192;
const MAX_DISCLOSURES: usize = 8_192;
const MAX_LIST: usize = 32_768;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramManifest {
    pub schema: String,
    pub system: SecurityProgramSystem,
    #[serde(default)]
    pub scopes: Vec<SecurityProgramScope>,
    #[serde(default)]
    pub campaigns: Vec<SecurityProgramCampaign>,
    #[serde(default)]
    pub findings: Vec<SecurityProgramFinding>,
    #[serde(default)]
    pub remediations: Vec<SecurityProgramRemediation>,
    #[serde(default)]
    pub incidents: Vec<SecurityProgramIncident>,
    #[serde(default)]
    pub disclosures: Vec<SecurityProgramDisclosure>,
    #[serde(default)]
    pub controls: SecurityProgramControls,
    #[serde(default)]
    pub policies: SecurityProgramPolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramSystem {
    pub id: String,
    pub version: String,
    pub owner: String,
    pub mission: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramScopeKind {
    Service,
    Api,
    Model,
    Dataset,
    Workflow,
    ResearchArtifact,
    Vendor,
    Organization,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramScope {
    pub id: String,
    pub name: String,
    pub kind: SecurityProgramScopeKind,
    pub target: String,
    pub owner: String,
    pub authorization_digest: Option<String>,
    #[serde(default)]
    pub allowed_methods: Vec<String>,
    #[serde(default)]
    pub forbidden_actions: Vec<String>,
    #[serde(default)]
    pub environments: Vec<String>,
    #[serde(default)]
    pub data_handling: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramCampaignStatus {
    Planned,
    Running,
    Completed,
    Stopped,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramCampaign {
    pub id: String,
    pub scope: String,
    pub operator: String,
    pub independent_reviewer: Option<String>,
    pub methodology: String,
    pub hypothesis: String,
    pub status: SecurityProgramCampaignStatus,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub evidence_digest: Option<String>,
    #[serde(default)]
    pub stop_conditions: Vec<String>,
    #[serde(default)]
    pub finding_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramFindingSeverity {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl SecurityProgramFindingSeverity {
    fn high_or_worse(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramFindingStatus {
    New,
    Triaged,
    Accepted,
    Remediated,
    Closed,
    FalsePositive,
    Duplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramFinding {
    pub id: String,
    pub campaign: String,
    pub title: String,
    pub severity: SecurityProgramFindingSeverity,
    pub status: SecurityProgramFindingStatus,
    pub evidence_digest: Option<String>,
    pub reproduction_digest: Option<String>,
    pub regression_digest: Option<String>,
    pub discovered_at: String,
    #[serde(default)]
    pub affected_targets: Vec<String>,
    #[serde(default)]
    pub remediation_ids: Vec<String>,
    pub incident_id: Option<String>,
    pub public_safe: bool,
    pub resolution_note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramRemediationStatus {
    Open,
    InProgress,
    Blocked,
    Complete,
    Waived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramRemediation {
    pub id: String,
    pub finding: String,
    pub owner: String,
    pub action: String,
    pub status: SecurityProgramRemediationStatus,
    pub due_at: String,
    pub verification_digest: Option<String>,
    pub rationale: Option<String>,
    pub approval_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramTimelineEvent {
    pub epoch: u64,
    pub actor: String,
    pub event: String,
    pub evidence_digest: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramIncidentStatus {
    Open,
    Contained,
    Closed,
    Accepted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramIncident {
    pub id: String,
    pub finding: String,
    pub severity: SecurityProgramFindingSeverity,
    pub owner: String,
    pub status: SecurityProgramIncidentStatus,
    pub opened_at: String,
    pub contained_at: Option<String>,
    pub closed_at: Option<String>,
    pub containment_evidence: Option<String>,
    pub closure_evidence: Option<String>,
    pub notification_required: bool,
    #[serde(default)]
    pub timeline: Vec<SecurityProgramTimelineEvent>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramDisclosureStage {
    Withheld,
    Internal,
    Advisory,
    Public,
}

impl SecurityProgramDisclosureStage {
    fn rank(self) -> u8 {
        match self {
            Self::Withheld => 0,
            Self::Internal => 1,
            Self::Advisory => 2,
            Self::Public => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramDisclosure {
    pub id: String,
    pub finding: String,
    pub stage: SecurityProgramDisclosureStage,
    pub audience: String,
    pub requested_at: String,
    pub approver: Option<String>,
    pub approval_digest: Option<String>,
    pub advisory_digest: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramControls {
    #[serde(default)]
    pub scope_authorization: bool,
    #[serde(default)]
    pub operator_separation: bool,
    #[serde(default)]
    pub independent_review: bool,
    #[serde(default)]
    pub evidence_retention: bool,
    #[serde(default)]
    pub remediation_tracking: bool,
    #[serde(default)]
    pub incident_response: bool,
    #[serde(default)]
    pub disclosure_review: bool,
    #[serde(default)]
    pub regression_testing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramPolicies {
    #[serde(default = "default_true")]
    pub require_scope_authorization: bool,
    #[serde(default = "default_true")]
    pub require_independent_review: bool,
    #[serde(default = "default_true")]
    pub require_campaign_evidence: bool,
    #[serde(default = "default_true")]
    pub require_finding_evidence: bool,
    #[serde(default = "default_true")]
    pub require_remediation: bool,
    #[serde(default = "default_true")]
    pub require_incident_for_high: bool,
    #[serde(default = "default_true")]
    pub require_disclosure_approval: bool,
    #[serde(default = "default_true")]
    pub require_regression_for_closed: bool,
    #[serde(default = "default_true")]
    pub require_controls: bool,
}

impl Default for SecurityProgramPolicies {
    fn default() -> Self {
        Self {
            require_scope_authorization: true,
            require_independent_review: true,
            require_campaign_evidence: true,
            require_finding_evidence: true,
            require_remediation: true,
            require_incident_for_high: true,
            require_disclosure_approval: true,
            require_regression_for_closed: true,
            require_controls: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityProgramIssueSeverity {
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramIssue {
    pub code: String,
    pub severity: SecurityProgramIssueSeverity,
    pub subject: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramScopeAudit {
    pub scope_id: String,
    pub authorization_valid: bool,
    pub methods_valid: bool,
    pub guardrails_valid: bool,
    pub environments_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramCampaignAudit {
    pub campaign_id: String,
    pub scope_valid: bool,
    pub operator_present: bool,
    pub independent_review_valid: bool,
    pub methodology_valid: bool,
    pub evidence_valid: bool,
    pub complete: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramFindingAudit {
    pub finding_id: String,
    pub campaign_valid: bool,
    pub evidence_valid: bool,
    pub reproduction_valid: bool,
    pub severity_requires_action: bool,
    pub remediation_valid: bool,
    pub incident_required: bool,
    pub incident_valid: bool,
    pub regression_present: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramRemediationAudit {
    pub remediation_id: String,
    pub finding_valid: bool,
    pub owner_valid: bool,
    pub completion_valid: bool,
    pub verification_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramIncidentAudit {
    pub incident_id: String,
    pub finding_valid: bool,
    pub timeline_valid: bool,
    pub containment_valid: bool,
    pub closure_valid: bool,
    pub notification_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramDisclosureAudit {
    pub disclosure_id: String,
    pub finding_valid: bool,
    pub stage_order_valid: bool,
    pub approval_valid: bool,
    pub advisory_valid: bool,
    pub publication_valid: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramControlAudit {
    pub control: String,
    pub enabled: bool,
    pub required: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramCounts {
    pub scopes: usize,
    pub authorized_scopes: usize,
    pub campaigns: usize,
    pub completed_campaigns: usize,
    pub findings: usize,
    pub high_or_worse_findings: usize,
    pub actionable_findings: usize,
    pub remediations: usize,
    pub completed_remediations: usize,
    pub incidents: usize,
    pub open_incidents: usize,
    pub closed_incidents: usize,
    pub disclosures: usize,
    pub advisory_disclosures: usize,
    pub public_disclosures: usize,
    pub enabled_controls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityProgramAudit {
    pub schema: String,
    pub manifest_schema: String,
    pub digest: String,
    pub valid: bool,
    pub system_id: String,
    pub counts: SecurityProgramCounts,
    pub scope_audits: Vec<SecurityProgramScopeAudit>,
    pub campaign_audits: Vec<SecurityProgramCampaignAudit>,
    pub finding_audits: Vec<SecurityProgramFindingAudit>,
    pub remediation_audits: Vec<SecurityProgramRemediationAudit>,
    pub incident_audits: Vec<SecurityProgramIncidentAudit>,
    pub disclosure_audits: Vec<SecurityProgramDisclosureAudit>,
    pub control_audits: Vec<SecurityProgramControlAudit>,
    pub issues: Vec<SecurityProgramIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SecurityProgramError {
    #[error("cannot canonicalize security program manifest: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize security program manifest: {0}")]
    Serialization(String),
}

impl SecurityProgramManifest {
    pub fn digest(&self) -> Result<ContentHash, SecurityProgramError> {
        let value = serde_json::to_value(self)
            .map_err(|error| SecurityProgramError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<SecurityProgramAudit, SecurityProgramError> {
        let digest = self.digest()?.to_string();
        let mut issues = Vec::new();
        let mut scopes = BTreeMap::<String, &SecurityProgramScope>::new();
        let mut campaigns = BTreeMap::<String, &SecurityProgramCampaign>::new();
        let mut findings = BTreeMap::<String, &SecurityProgramFinding>::new();
        let mut remediations = BTreeMap::<String, &SecurityProgramRemediation>::new();
        let mut incidents = BTreeMap::<String, &SecurityProgramIncident>::new();
        let mut disclosures = BTreeMap::<String, &SecurityProgramDisclosure>::new();

        bound(&mut issues, "scopes", self.scopes.len(), MAX_SCOPES);
        bound(
            &mut issues,
            "campaigns",
            self.campaigns.len(),
            MAX_CAMPAIGNS,
        );
        bound(&mut issues, "findings", self.findings.len(), MAX_FINDINGS);
        bound(
            &mut issues,
            "remediations",
            self.remediations.len(),
            MAX_REMEDIATIONS,
        );
        bound(
            &mut issues,
            "incidents",
            self.incidents.len(),
            MAX_INCIDENTS,
        );
        bound(
            &mut issues,
            "disclosures",
            self.disclosures.len(),
            MAX_DISCLOSURES,
        );
        if self.schema != SECURITY_PROGRAM_MANIFEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "manifest",
                format!(
                    "expected {SECURITY_PROGRAM_MANIFEST_SCHEMA}, got {}",
                    self.schema
                ),
                "regenerate the declaration with the published security-program schema",
            );
        }
        for (field, value) in [
            ("system.id", &self.system.id),
            ("system.version", &self.system.version),
            ("system.owner", &self.system.owner),
            ("system.mission", &self.system.mission),
        ] {
            if value.trim().is_empty() {
                blocking(
                    &mut issues,
                    "required_field_empty",
                    field,
                    format!("{field} is empty"),
                    "declare the program identity, accountable owner, version, and mission",
                );
            }
        }

        for scope in &self.scopes {
            if !insert_unique(&mut scopes, &scope.id, "scope", &mut issues) {
                continue;
            }
            scopes.insert(scope.id.clone(), scope);
            let authorization_valid = scope
                .authorization_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            let methods_valid = !scope.allowed_methods.is_empty()
                && bounded_strings(&scope.allowed_methods, "scope.allowed_methods", &mut issues);
            let guardrails_valid = !scope.forbidden_actions.is_empty()
                && bounded_strings(
                    &scope.forbidden_actions,
                    "scope.forbidden_actions",
                    &mut issues,
                )
                && scope.forbidden_actions.iter().all(|forbidden| {
                    !scope
                        .allowed_methods
                        .iter()
                        .any(|allowed| allowed == forbidden)
                });
            let environments_valid = !scope.environments.is_empty()
                && bounded_strings(&scope.environments, "scope.environments", &mut issues);
            if scope.id.trim().is_empty()
                || scope.name.trim().is_empty()
                || scope.target.trim().is_empty()
                || scope.owner.trim().is_empty()
            {
                blocking(
                    &mut issues,
                    "scope_incomplete",
                    &scope.id,
                    "scope id, name, target, and owner are required",
                    "declare the exact governed target and accountable owner",
                );
            }
            if self.policies.require_scope_authorization && !authorization_valid {
                blocking(
                    &mut issues,
                    "scope_authorization_missing",
                    &scope.id,
                    "scope has no valid authorization digest",
                    "bind a content-addressed authorization record before testing",
                );
            }
            if !methods_valid {
                blocking(
                    &mut issues,
                    "scope_methods_missing",
                    &scope.id,
                    "scope has no bounded allowed test methods",
                    "declare at least one finite method and avoid wildcard actions",
                );
            }
            if !guardrails_valid {
                blocking(
                    &mut issues,
                    "scope_guardrails_invalid",
                    &scope.id,
                    "scope guardrails are missing or overlap an allowed method",
                    "declare explicit forbidden actions that cannot be smuggled into the scope",
                );
            }
            if !environments_valid {
                blocking(
                    &mut issues,
                    "scope_environments_missing",
                    &scope.id,
                    "scope does not identify a bounded environment",
                    "name the staging, isolated, or other explicitly authorized environment",
                );
            }
            if scope
                .data_handling
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                warning(
                    &mut issues,
                    "scope_data_handling_unspecified",
                    &scope.id,
                    "scope does not describe how sensitive test data is handled",
                    "state whether fixtures are synthetic, minimized, or access-controlled",
                );
            }
        }
        if self.scopes.is_empty() {
            blocking(
                &mut issues,
                "scopes_missing",
                "scopes",
                "the program declares no authorized testing scope",
                "inventory the targets and bind each to a finite authorization record",
            );
        }

        for campaign in &self.campaigns {
            if !insert_unique(&mut campaigns, &campaign.id, "campaign", &mut issues) {
                continue;
            }
            campaigns.insert(campaign.id.clone(), campaign);
            let scope_valid = scopes.contains_key(&campaign.scope);
            let operator_present = !campaign.operator.trim().is_empty();
            let independent_review_valid = campaign
                .independent_reviewer
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|reviewer| reviewer != campaign.operator && reviewer != self.system.owner)
                .unwrap_or(false);
            let methodology_valid = !campaign.methodology.trim().is_empty()
                && !campaign.hypothesis.trim().is_empty()
                && !campaign.stop_conditions.is_empty()
                && bounded_strings(
                    &campaign.stop_conditions,
                    "campaign.stop_conditions",
                    &mut issues,
                );
            let evidence_valid = campaign
                .evidence_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            let complete = campaign.status == SecurityProgramCampaignStatus::Completed;
            if !scope_valid {
                blocking(
                    &mut issues,
                    "campaign_scope_missing",
                    &campaign.id,
                    format!("campaign names undeclared scope {}", campaign.scope),
                    "bind every campaign to an authorized scope",
                );
            }
            if !operator_present {
                blocking(
                    &mut issues,
                    "campaign_operator_missing",
                    &campaign.id,
                    "campaign has no accountable operator",
                    "name the operator or team responsible for the campaign",
                );
            }
            if self.policies.require_independent_review && !independent_review_valid {
                blocking(
                    &mut issues,
                    "campaign_independent_review_missing",
                    &campaign.id,
                    "campaign reviewer is missing or not independent of the operator and owner",
                    "assign a separate reviewer before accepting campaign evidence",
                );
            }
            if !methodology_valid {
                blocking(
                    &mut issues,
                    "campaign_methodology_missing",
                    &campaign.id,
                    "campaign methodology, hypothesis, or stop conditions are incomplete",
                    "bound the test method, expected failure, and conditions that stop activity",
                );
            }
            if self.policies.require_campaign_evidence
                && (!evidence_valid || (complete && campaign.completed_at.is_none()))
            {
                blocking(
                    &mut issues,
                    "campaign_evidence_missing",
                    &campaign.id,
                    "campaign lacks a valid evidence digest or completion timestamp",
                    "bind immutable campaign evidence and record when the campaign completed",
                );
            }
            if campaign.finding_ids.len() > MAX_LIST {
                bound(
                    &mut issues,
                    "campaign.finding_ids",
                    campaign.finding_ids.len(),
                    MAX_LIST,
                );
            }
        }

        for finding in &self.findings {
            if !insert_unique(&mut findings, &finding.id, "finding", &mut issues) {
                continue;
            }
            findings.insert(finding.id.clone(), finding);
            let campaign_valid = campaigns.contains_key(&finding.campaign);
            let evidence_valid = finding
                .evidence_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            let reproduction_valid = finding
                .reproduction_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            let severity_requires_action = finding.severity.high_or_worse();
            let remediation_valid = !finding.remediation_ids.is_empty();
            let incident_required =
                self.policies.require_incident_for_high && severity_requires_action;
            let incident_valid = finding
                .incident_id
                .as_deref()
                .map(|id| self.incidents.iter().any(|incident| incident.id == id))
                .unwrap_or(false);
            let regression_present = finding
                .regression_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            if !campaign_valid {
                blocking(
                    &mut issues,
                    "finding_campaign_missing",
                    &finding.id,
                    format!("finding names undeclared campaign {}", finding.campaign),
                    "bind every finding to the campaign that produced its evidence",
                );
            }
            if finding.title.trim().is_empty()
                || finding.discovered_at.trim().is_empty()
                || finding.affected_targets.is_empty()
            {
                blocking(
                    &mut issues,
                    "finding_incomplete",
                    &finding.id,
                    "finding title, discovery time, and affected targets are required",
                    "retain a bounded description of what was observed and where",
                );
            }
            if self.policies.require_finding_evidence && !evidence_valid {
                blocking(
                    &mut issues,
                    "finding_evidence_missing",
                    &finding.id,
                    "finding has no valid content-addressed evidence",
                    "bind the observation to immutable evidence before triage",
                );
            }
            if severity_requires_action
                && self.policies.require_finding_evidence
                && !reproduction_valid
            {
                blocking(
                    &mut issues,
                    "finding_reproduction_missing",
                    &finding.id,
                    "high or critical finding has no reproducibility digest",
                    "retain a minimized reproduction or explicitly refuse the finding",
                );
            }
            if self.policies.require_remediation && severity_requires_action && !remediation_valid {
                blocking(
                    &mut issues,
                    "finding_remediation_missing",
                    &finding.id,
                    "high or critical finding has no remediation link",
                    "create a tracked remediation or record a bounded accepted-risk decision",
                );
            }
            if incident_required && !incident_valid {
                blocking(
                    &mut issues,
                    "finding_incident_missing",
                    &finding.id,
                    "high or critical finding has no incident response record",
                    "open an incident record with containment and closure evidence",
                );
            }
            if self.policies.require_regression_for_closed
                && finding.status == SecurityProgramFindingStatus::Closed
                && !regression_present
            {
                blocking(
                    &mut issues,
                    "finding_regression_missing",
                    &finding.id,
                    "closed finding has no regression-test digest",
                    "bind a post-remediation regression witness before closing the finding",
                );
            }
            if matches!(
                finding.status,
                SecurityProgramFindingStatus::Accepted
                    | SecurityProgramFindingStatus::FalsePositive
                    | SecurityProgramFindingStatus::Duplicate
            ) && finding
                .resolution_note
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none()
            {
                blocking(
                    &mut issues,
                    "finding_resolution_note_missing",
                    &finding.id,
                    "resolved or accepted finding has no decision record",
                    "state why the finding is accepted, duplicate, or a false positive",
                );
            }
        }

        for remediation in &self.remediations {
            if !insert_unique(
                &mut remediations,
                &remediation.id,
                "remediation",
                &mut issues,
            ) {
                continue;
            }
            remediations.insert(remediation.id.clone(), remediation);
            let finding_valid = findings.contains_key(&remediation.finding);
            let owner_valid = !remediation.owner.trim().is_empty()
                && !remediation.action.trim().is_empty()
                && !remediation.due_at.trim().is_empty();
            let complete = remediation.status == SecurityProgramRemediationStatus::Complete;
            let verification_valid = remediation
                .verification_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            if !finding_valid {
                blocking(
                    &mut issues,
                    "remediation_finding_missing",
                    &remediation.id,
                    format!(
                        "remediation names undeclared finding {}",
                        remediation.finding
                    ),
                    "bind each action to a finding in this program",
                );
            }
            if !owner_valid {
                blocking(
                    &mut issues,
                    "remediation_incomplete",
                    &remediation.id,
                    "remediation owner, action, and due time are required",
                    "assign an accountable owner and bounded action deadline",
                );
            }
            if complete && !verification_valid {
                blocking(
                    &mut issues,
                    "remediation_verification_missing",
                    &remediation.id,
                    "complete remediation has no valid verification digest",
                    "attach independent post-change verification before marking complete",
                );
            }
            if remediation.status == SecurityProgramRemediationStatus::Waived {
                let rationale_valid = remediation
                    .rationale
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_some();
                let approval_valid = remediation
                    .approval_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                if !rationale_valid || !approval_valid {
                    blocking(
                        &mut issues,
                        "remediation_waiver_missing",
                        &remediation.id,
                        "waived remediation lacks a rationale or approval digest",
                        "record bounded residual risk and independent approval",
                    );
                }
            }
        }

        for campaign in &self.campaigns {
            for finding_id in &campaign.finding_ids {
                match findings.get(finding_id) {
                    None => blocking(
                        &mut issues,
                        "campaign_finding_unknown",
                        &campaign.id,
                        format!("campaign references unknown finding {finding_id}"),
                        "bind campaign outputs only to declared findings",
                    ),
                    Some(finding) if finding.campaign != campaign.id => blocking(
                        &mut issues,
                        "campaign_finding_backlink_mismatch",
                        &campaign.id,
                        format!(
                            "campaign lists finding {finding_id}, but the finding names campaign {}",
                            finding.campaign
                        ),
                        "make campaign.finding_ids and finding.campaign agree",
                    ),
                    Some(_) => {}
                }
            }
        }
        for finding in &self.findings {
            for remediation_id in &finding.remediation_ids {
                match remediations.get(remediation_id) {
                    None => blocking(
                        &mut issues,
                        "finding_remediation_unknown",
                        &finding.id,
                        format!("finding references unknown remediation {remediation_id}"),
                        "bind finding closure to a declared remediation row",
                    ),
                    Some(remediation) if remediation.finding != finding.id => blocking(
                        &mut issues,
                        "finding_remediation_backlink_mismatch",
                        &finding.id,
                        format!(
                            "finding lists remediation {remediation_id}, but the remediation names finding {}",
                            remediation.finding
                        ),
                        "make finding.remediation_ids and remediation.finding agree",
                    ),
                    Some(_) => {}
                }
            }
        }
        for finding in &self.findings {
            if let Some(campaign) = campaigns.get(&finding.campaign) {
                if !campaign.finding_ids.iter().any(|id| id == &finding.id) {
                    blocking(
                        &mut issues,
                        "finding_campaign_backlink_missing",
                        &finding.id,
                        format!(
                            "finding names campaign {}, but the campaign does not list the finding",
                            finding.campaign
                        ),
                        "make campaign.finding_ids and finding.campaign agree",
                    );
                }
            }
        }

        for incident in &self.incidents {
            if !insert_unique(&mut incidents, &incident.id, "incident", &mut issues) {
                continue;
            }
            incidents.insert(incident.id.clone(), incident);
        }
        for finding in &self.findings {
            if let Some(incident_id) = &finding.incident_id {
                if !incidents.contains_key(incident_id) {
                    blocking(
                        &mut issues,
                        "finding_incident_unknown",
                        &finding.id,
                        format!("finding references unknown incident {incident_id}"),
                        "bind the finding to a declared incident or remove the stale reference",
                    );
                }
            }
        }
        let incident_audits = self
            .incidents
            .iter()
            .map(|incident| {
                let finding_valid = findings.contains_key(&incident.finding);
                let timeline_valid = timeline_valid(&incident.timeline, &mut issues, &incident.id);
                let containment_valid = incident
                    .containment_evidence
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false)
                    && incident
                        .contained_at
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_some();
                let closure_valid = incident
                    .closure_evidence
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false)
                    && incident
                        .closed_at
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .is_some();
                let notification_valid = !incident.notification_required || timeline_valid;
                if !finding_valid {
                    blocking(
                        &mut issues,
                        "incident_finding_missing",
                        &incident.id,
                        format!("incident names undeclared finding {}", incident.finding),
                        "bind the incident to its triggering finding",
                    );
                }
                if !timeline_valid {
                    blocking(
                        &mut issues,
                        "incident_timeline_invalid",
                        &incident.id,
                        "incident timeline is empty, unordered, or incomplete",
                        "retain an append-only timeline with actor, event, and evidence",
                    );
                }
                if matches!(
                    incident.status,
                    SecurityProgramIncidentStatus::Contained
                        | SecurityProgramIncidentStatus::Closed
                ) && !containment_valid
                {
                    blocking(
                        &mut issues,
                        "incident_containment_missing",
                        &incident.id,
                        "contained incident lacks timestamp or evidence",
                        "bind a content-addressed containment witness",
                    );
                }
                if incident.status == SecurityProgramIncidentStatus::Closed && !closure_valid {
                    blocking(
                        &mut issues,
                        "incident_closure_missing",
                        &incident.id,
                        "closed incident lacks timestamp or closure evidence",
                        "record independent closure evidence before closing",
                    );
                }
                if !notification_valid {
                    blocking(
                        &mut issues,
                        "incident_notification_missing",
                        &incident.id,
                        "incident requiring notification has no complete timeline",
                        "record the notification decision as part of the incident chain",
                    );
                }
                SecurityProgramIncidentAudit {
                    incident_id: incident.id.clone(),
                    finding_valid,
                    timeline_valid,
                    containment_valid,
                    closure_valid,
                    notification_valid,
                    ready: finding_valid
                        && timeline_valid
                        && (!matches!(
                            incident.status,
                            SecurityProgramIncidentStatus::Contained
                                | SecurityProgramIncidentStatus::Closed
                        ) || containment_valid)
                        && (incident.status != SecurityProgramIncidentStatus::Closed
                            || closure_valid)
                        && notification_valid,
                }
            })
            .collect::<Vec<_>>();

        for disclosure in &self.disclosures {
            if !insert_unique(&mut disclosures, &disclosure.id, "disclosure", &mut issues) {
                continue;
            }
            disclosures.insert(disclosure.id.clone(), disclosure);
        }
        let mut disclosure_stages = BTreeMap::<String, Vec<SecurityProgramDisclosureStage>>::new();
        for disclosure in &self.disclosures {
            disclosure_stages
                .entry(disclosure.finding.clone())
                .or_default()
                .push(disclosure.stage);
        }
        let disclosure_audits = self.disclosures.iter().map(|disclosure| {
            let finding_valid = findings.contains_key(&disclosure.finding);
            let stages = disclosure_stages.get(&disclosure.finding).cloned().unwrap_or_default();
            let mut ordered = stages.clone();
            ordered.sort_by_key(|stage| stage.rank());
            let stage_order_valid = stages == ordered && match disclosure.stage {
                SecurityProgramDisclosureStage::Public => stages.contains(&SecurityProgramDisclosureStage::Advisory),
                _ => true,
            };
            let approval_valid = disclosure.approver.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some()
                && disclosure.approval_digest.as_deref().map(valid_digest).unwrap_or(false);
            let advisory_valid = disclosure.stage != SecurityProgramDisclosureStage::Public || disclosure.advisory_digest.as_deref().map(valid_digest).unwrap_or(false);
            let publication_valid = disclosure.stage != SecurityProgramDisclosureStage::Public || disclosure.published_at.as_deref().map(str::trim).filter(|value| !value.is_empty()).is_some();
            if !finding_valid {
                blocking(&mut issues, "disclosure_finding_missing", &disclosure.id, format!("disclosure names undeclared finding {}", disclosure.finding), "bind each disclosure to a finding");
            }
            if disclosure.audience.trim().is_empty() || disclosure.requested_at.trim().is_empty() {
                blocking(&mut issues, "disclosure_incomplete", &disclosure.id, "disclosure audience and request time are required", "retain the recipient scope and request timestamp");
            }
            if !stage_order_valid {
                blocking(&mut issues, "disclosure_stage_order_invalid", &disclosure.id, "disclosure stages are not sequential or public has no advisory predecessor", "advance disclosure through internal review and advisory evidence before publication");
            }
            if self.policies.require_disclosure_approval && disclosure.stage != SecurityProgramDisclosureStage::Withheld && !approval_valid {
                blocking(&mut issues, "disclosure_approval_missing", &disclosure.id, "disclosure lacks an approver or valid approval digest", "bind independent disclosure approval before delivery");
            }
            if !advisory_valid {
                blocking(&mut issues, "disclosure_advisory_missing", &disclosure.id, "public disclosure has no advisory digest", "bind the reviewed advisory artifact before publication");
            }
            if !publication_valid {
                blocking(&mut issues, "disclosure_publication_missing", &disclosure.id, "public disclosure has no publication timestamp", "record publication only after the approved artifact exists");
            }
            let finding_safe = findings.get(&disclosure.finding).map(|finding| finding.public_safe).unwrap_or(false);
            if disclosure.stage == SecurityProgramDisclosureStage::Public && !finding_safe {
                blocking(&mut issues, "disclosure_finding_not_public_safe", &disclosure.id, "public disclosure references a finding not marked safe for publication", "complete privacy, safety, and affected-party review before publication");
            }
            SecurityProgramDisclosureAudit { disclosure_id: disclosure.id.clone(), finding_valid, stage_order_valid, approval_valid, advisory_valid, publication_valid, ready: finding_valid && stage_order_valid && (disclosure.stage == SecurityProgramDisclosureStage::Withheld || approval_valid) && advisory_valid && publication_valid && (disclosure.stage != SecurityProgramDisclosureStage::Public || finding_safe) }
        }).collect::<Vec<_>>();

        let scope_audits = self
            .scopes
            .iter()
            .map(|scope| {
                let authorization_valid = scope
                    .authorization_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                let methods_valid = !scope.allowed_methods.is_empty()
                    && scope
                        .allowed_methods
                        .iter()
                        .all(|method| bounded_string(method));
                let guardrails_valid = !scope.forbidden_actions.is_empty()
                    && scope
                        .forbidden_actions
                        .iter()
                        .all(|forbidden| bounded_string(forbidden))
                    && scope
                        .forbidden_actions
                        .iter()
                        .all(|forbidden| !scope.allowed_methods.contains(forbidden));
                let environments_valid = !scope.environments.is_empty()
                    && scope
                        .environments
                        .iter()
                        .all(|environment| bounded_string(environment));
                SecurityProgramScopeAudit {
                    scope_id: scope.id.clone(),
                    authorization_valid,
                    methods_valid,
                    guardrails_valid,
                    environments_valid,
                    ready: authorization_valid
                        && methods_valid
                        && guardrails_valid
                        && environments_valid,
                }
            })
            .collect::<Vec<_>>();
        let campaign_audits = self
            .campaigns
            .iter()
            .map(|campaign| {
                let scope_valid = scopes.contains_key(&campaign.scope);
                let operator_present = !campaign.operator.trim().is_empty();
                let independent_review_valid = campaign
                    .independent_reviewer
                    .as_deref()
                    .map(|reviewer| {
                        !reviewer.trim().is_empty()
                            && reviewer != campaign.operator
                            && reviewer != self.system.owner
                    })
                    .unwrap_or(false);
                let methodology_valid = !campaign.methodology.trim().is_empty()
                    && !campaign.hypothesis.trim().is_empty()
                    && !campaign.stop_conditions.is_empty();
                let evidence_valid = campaign
                    .evidence_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                let complete = campaign.status == SecurityProgramCampaignStatus::Completed;
                SecurityProgramCampaignAudit {
                    campaign_id: campaign.id.clone(),
                    scope_valid,
                    operator_present,
                    independent_review_valid,
                    methodology_valid,
                    evidence_valid,
                    complete,
                    ready: scope_valid
                        && operator_present
                        && independent_review_valid
                        && methodology_valid
                        && evidence_valid
                        && (!complete || campaign.completed_at.is_some()),
                }
            })
            .collect::<Vec<_>>();
        let remediation_audits = self
            .remediations
            .iter()
            .map(|remediation| {
                let finding_valid = findings.get(&remediation.finding).is_some_and(|finding| {
                    finding
                        .remediation_ids
                        .iter()
                        .any(|id| id == &remediation.id)
                });
                let owner_valid = !remediation.owner.trim().is_empty()
                    && !remediation.action.trim().is_empty()
                    && !remediation.due_at.trim().is_empty();
                let completion_valid = remediation.status
                    != SecurityProgramRemediationStatus::Complete
                    || remediation
                        .verification_digest
                        .as_deref()
                        .map(valid_digest)
                        .unwrap_or(false);
                let verification_valid = remediation
                    .verification_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                SecurityProgramRemediationAudit {
                    remediation_id: remediation.id.clone(),
                    finding_valid,
                    owner_valid,
                    completion_valid,
                    verification_valid,
                    ready: finding_valid && owner_valid && completion_valid,
                }
            })
            .collect::<Vec<_>>();
        let finding_audits = self
            .findings
            .iter()
            .map(|finding| {
                let campaign_valid = campaigns.get(&finding.campaign).is_some_and(|campaign| {
                    campaign.finding_ids.iter().any(|id| id == &finding.id)
                });
                let evidence_valid = finding
                    .evidence_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                let reproduction_valid = finding
                    .reproduction_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                let severity_requires_action = finding.severity.high_or_worse();
                let remediation_valid = finding.remediation_ids.iter().all(|id| {
                    remediations
                        .get(id)
                        .is_some_and(|remediation| remediation.finding == finding.id)
                }) && (!severity_requires_action
                    || !finding.remediation_ids.is_empty());
                let incident_required =
                    self.policies.require_incident_for_high && severity_requires_action;
                let incident_valid = finding
                    .incident_id
                    .as_deref()
                    .map(|id| incidents.contains_key(id))
                    .unwrap_or(false);
                let regression_present = finding
                    .regression_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                SecurityProgramFindingAudit {
                    finding_id: finding.id.clone(),
                    campaign_valid,
                    evidence_valid,
                    reproduction_valid,
                    severity_requires_action,
                    remediation_valid,
                    incident_required,
                    incident_valid,
                    regression_present,
                    ready: campaign_valid
                        && evidence_valid
                        && (!severity_requires_action || reproduction_valid)
                        && (!self.policies.require_remediation
                            || !severity_requires_action
                            || remediation_valid)
                        && (!incident_required || incident_valid)
                        && (finding.status != SecurityProgramFindingStatus::Closed
                            || !self.policies.require_regression_for_closed
                            || regression_present),
                }
            })
            .collect::<Vec<_>>();
        let control_audits = controls_from(&self.controls);
        if self.policies.require_controls {
            for control in &control_audits {
                if control.required && !control.enabled {
                    blocking(
                        &mut issues,
                        "required_control_disabled",
                        &control.control,
                        format!("required program control {} is disabled", control.control),
                        "enable the control or narrow the program declaration",
                    );
                }
            }
        }
        let counts = SecurityProgramCounts {
            scopes: self.scopes.len(),
            authorized_scopes: scope_audits.iter().filter(|row| row.ready).count(),
            campaigns: self.campaigns.len(),
            completed_campaigns: self
                .campaigns
                .iter()
                .filter(|campaign| campaign.status == SecurityProgramCampaignStatus::Completed)
                .count(),
            findings: self.findings.len(),
            high_or_worse_findings: self
                .findings
                .iter()
                .filter(|finding| finding.severity.high_or_worse())
                .count(),
            actionable_findings: self
                .findings
                .iter()
                .filter(|finding| {
                    matches!(
                        finding.status,
                        SecurityProgramFindingStatus::New | SecurityProgramFindingStatus::Triaged
                    )
                })
                .count(),
            remediations: self.remediations.len(),
            completed_remediations: self
                .remediations
                .iter()
                .filter(|remediation| {
                    remediation.status == SecurityProgramRemediationStatus::Complete
                })
                .count(),
            incidents: self.incidents.len(),
            open_incidents: self
                .incidents
                .iter()
                .filter(|incident| {
                    matches!(
                        incident.status,
                        SecurityProgramIncidentStatus::Open
                            | SecurityProgramIncidentStatus::Contained
                    )
                })
                .count(),
            closed_incidents: self
                .incidents
                .iter()
                .filter(|incident| incident.status == SecurityProgramIncidentStatus::Closed)
                .count(),
            disclosures: self.disclosures.len(),
            advisory_disclosures: self
                .disclosures
                .iter()
                .filter(|disclosure| disclosure.stage == SecurityProgramDisclosureStage::Advisory)
                .count(),
            public_disclosures: self
                .disclosures
                .iter()
                .filter(|disclosure| disclosure.stage == SecurityProgramDisclosureStage::Public)
                .count(),
            enabled_controls: control_audits
                .iter()
                .filter(|control| control.enabled)
                .count(),
        };
        issues.sort_by(|left, right| {
            (
                left.code.as_str(),
                left.subject.as_str(),
                left.detail.as_str(),
            )
                .cmp(&(
                    right.code.as_str(),
                    right.subject.as_str(),
                    right.detail.as_str(),
                ))
        });
        let valid = !issues
            .iter()
            .any(|issue| issue.severity == SecurityProgramIssueSeverity::Blocking);
        Ok(SecurityProgramAudit {
            schema: SECURITY_PROGRAM_AUDIT_SCHEMA.into(),
            manifest_schema: self.schema.clone(),
            digest,
            valid,
            system_id: self.system.id.clone(),
            counts,
            scope_audits,
            campaign_audits,
            finding_audits,
            remediation_audits,
            incident_audits,
            disclosure_audits,
            control_audits,
            issues,
            guarantees: vec![
                "authorized scope, campaign independence, evidence, findings, remediation, incidents, disclosure, and regression controls remain separate layers".into(),
                "high and critical findings cannot become ready without evidence, action, incident linkage, and bounded closure".into(),
                "blocking posture is derived from deterministic issue rows rather than caller-supplied readiness".into(),
            ],
            limitations: vec![
                "the audit does not run scanners, fuzzers, probes, sandboxed code, or containment actions".into(),
                "the audit does not contact vendors, publish disclosures, mutate incidents, or verify a live control".into(),
                "all scope, evidence, timestamps, approvals, and control states are caller-supplied declarations".into(),
            ],
        })
    }
}

fn controls_from(controls: &SecurityProgramControls) -> Vec<SecurityProgramControlAudit> {
    [
        ("scope_authorization", controls.scope_authorization),
        ("operator_separation", controls.operator_separation),
        ("independent_review", controls.independent_review),
        ("evidence_retention", controls.evidence_retention),
        ("remediation_tracking", controls.remediation_tracking),
        ("incident_response", controls.incident_response),
        ("disclosure_review", controls.disclosure_review),
        ("regression_testing", controls.regression_testing),
    ]
    .into_iter()
    .map(|(control, enabled)| SecurityProgramControlAudit {
        control: control.into(),
        enabled,
        required: true,
        ready: enabled,
    })
    .collect()
}

fn timeline_valid(
    events: &[SecurityProgramTimelineEvent],
    issues: &mut Vec<SecurityProgramIssue>,
    subject: &str,
) -> bool {
    if events.is_empty() || events.len() > MAX_LIST {
        if events.len() > MAX_LIST {
            bound(issues, "incident.timeline", events.len(), MAX_LIST);
        }
        return false;
    }
    let mut previous = None;
    for event in events {
        let evidence_valid = event
            .evidence_digest
            .as_deref()
            .map(valid_digest)
            .unwrap_or(false);
        if event.actor.trim().is_empty()
            || event.event.trim().is_empty()
            || !evidence_valid
            || previous.is_some_and(|value| event.epoch <= value)
        {
            warning(
                issues,
                "incident_timeline_row_invalid",
                subject,
                "incident timeline contains an empty, unordered, or unbound event",
                "retain strictly increasing event epochs with actor, action, and evidence",
            );
            return false;
        }
        previous = Some(event.epoch);
    }
    true
}

fn bounded_string(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 512
        && !trimmed.chars().any(char::is_control)
        && !trimmed.contains('*')
        && !trimmed.contains("..")
}

fn bounded_strings(values: &[String], field: &str, issues: &mut Vec<SecurityProgramIssue>) -> bool {
    if values.len() > MAX_LIST {
        bound(issues, field, values.len(), MAX_LIST);
    }
    values.iter().all(|value| bounded_string(value))
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn insert_unique<T>(
    map: &mut BTreeMap<String, &T>,
    id: &str,
    kind: &str,
    issues: &mut Vec<SecurityProgramIssue>,
) -> bool {
    if map.contains_key(id) {
        blocking(
            issues,
            "duplicate_id",
            id,
            format!("duplicate {kind} identifier {id}"),
            format!("retain one canonical {kind} row for {id}"),
        );
        false
    } else {
        true
    }
}

fn bound(issues: &mut Vec<SecurityProgramIssue>, field: &str, actual: usize, maximum: usize) {
    if actual > maximum {
        blocking(
            issues,
            "bound_exceeded",
            field,
            format!("{field} contains {actual} rows, above the bound {maximum}"),
            format!("split or reduce {field} to at most {maximum} rows"),
        );
    }
}

fn blocking(
    issues: &mut Vec<SecurityProgramIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(SecurityProgramIssue {
        code: code.into(),
        severity: SecurityProgramIssueSeverity::Blocking,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

fn warning(
    issues: &mut Vec<SecurityProgramIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(SecurityProgramIssue {
        code: code.into(),
        severity: SecurityProgramIssueSeverity::Warning,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> SecurityProgramManifest {
        let digest = || Some("a".repeat(64));
        SecurityProgramManifest {
            schema: SECURITY_PROGRAM_MANIFEST_SCHEMA.into(),
            system: SecurityProgramSystem {
                id: "aurora-security".into(),
                version: "0.1.0".into(),
                owner: "security-owner".into(),
                mission: "bounded adversarial assurance".into(),
            },
            scopes: vec![SecurityProgramScope {
                id: "api-staging".into(),
                name: "staging API".into(),
                kind: SecurityProgramScopeKind::Api,
                target: "api-staging.internal".into(),
                owner: "service-owner".into(),
                authorization_digest: digest(),
                allowed_methods: vec!["authenticated-read".into(), "rate-limited-input".into()],
                forbidden_actions: vec![
                    "production-write".into(),
                    "credential-exfiltration".into(),
                ],
                environments: vec!["isolated-staging".into()],
                data_handling: Some("synthetic fixtures only".into()),
            }],
            campaigns: vec![SecurityProgramCampaign {
                id: "campaign-1".into(),
                scope: "api-staging".into(),
                operator: "red-team".into(),
                independent_reviewer: Some("independent-reviewer".into()),
                methodology: "bounded mutation and manual review".into(),
                hypothesis: "invalid input can cross a trust boundary".into(),
                status: SecurityProgramCampaignStatus::Completed,
                started_at: Some("2026-01-01".into()),
                completed_at: Some("2026-01-02".into()),
                evidence_digest: digest(),
                stop_conditions: vec!["stop on production boundary".into()],
                finding_ids: vec!["finding-1".into()],
            }],
            findings: vec![SecurityProgramFinding {
                id: "finding-1".into(),
                campaign: "campaign-1".into(),
                title: "boundary mismatch".into(),
                severity: SecurityProgramFindingSeverity::High,
                status: SecurityProgramFindingStatus::Closed,
                evidence_digest: digest(),
                reproduction_digest: digest(),
                regression_digest: digest(),
                discovered_at: "2026-01-02".into(),
                affected_targets: vec!["api-staging".into()],
                remediation_ids: vec!["remediation-1".into()],
                incident_id: Some("incident-1".into()),
                public_safe: true,
                resolution_note: None,
            }],
            remediations: vec![SecurityProgramRemediation {
                id: "remediation-1".into(),
                finding: "finding-1".into(),
                owner: "service-owner".into(),
                action: "validate boundary before dispatch".into(),
                status: SecurityProgramRemediationStatus::Complete,
                due_at: "2026-01-10".into(),
                verification_digest: digest(),
                rationale: None,
                approval_digest: None,
            }],
            incidents: vec![SecurityProgramIncident {
                id: "incident-1".into(),
                finding: "finding-1".into(),
                severity: SecurityProgramFindingSeverity::High,
                owner: "incident-owner".into(),
                status: SecurityProgramIncidentStatus::Closed,
                opened_at: "2026-01-02".into(),
                contained_at: Some("2026-01-02".into()),
                closed_at: Some("2026-01-03".into()),
                containment_evidence: digest(),
                closure_evidence: digest(),
                notification_required: true,
                timeline: vec![
                    SecurityProgramTimelineEvent {
                        epoch: 1,
                        actor: "incident-owner".into(),
                        event: "incident opened".into(),
                        evidence_digest: digest(),
                    },
                    SecurityProgramTimelineEvent {
                        epoch: 2,
                        actor: "incident-owner".into(),
                        event: "containment verified".into(),
                        evidence_digest: digest(),
                    },
                ],
            }],
            disclosures: vec![SecurityProgramDisclosure {
                id: "advisory-1".into(),
                finding: "finding-1".into(),
                stage: SecurityProgramDisclosureStage::Advisory,
                audience: "affected operators".into(),
                requested_at: "2026-01-04".into(),
                approver: Some("independent-reviewer".into()),
                approval_digest: digest(),
                advisory_digest: digest(),
                published_at: Some("2026-01-04".into()),
            }],
            controls: SecurityProgramControls {
                scope_authorization: true,
                operator_separation: true,
                independent_review: true,
                evidence_retention: true,
                remediation_tracking: true,
                incident_response: true,
                disclosure_review: true,
                regression_testing: true,
            },
            policies: SecurityProgramPolicies::default(),
        }
    }

    #[test]
    fn valid_program_keeps_scope_campaign_finding_remediation_incident_and_disclosure_layers() {
        let report = manifest().audit().expect("audit");
        assert!(report.valid, "issues: {:?}", report.issues);
        assert_eq!(report.counts.authorized_scopes, 1);
        assert_eq!(report.counts.completed_remediations, 1);
        assert!(report.finding_audits[0].incident_valid);
        assert!(report.incident_audits[0].closure_valid);
        assert!(report.disclosure_audits[0].approval_valid);
    }

    #[test]
    fn missing_authorization_review_evidence_and_closure_fail_closed() {
        let mut value = manifest();
        value.scopes[0].authorization_digest = None;
        value.campaigns[0].independent_reviewer = None;
        value.findings[0].evidence_digest = None;
        value.remediations[0].verification_digest = None;
        value.incidents[0].closure_evidence = None;
        value.controls.disclosure_review = false;
        let report = value.audit().expect("audit");
        assert!(!report.valid);
        for code in [
            "scope_authorization_missing",
            "campaign_independent_review_missing",
            "finding_evidence_missing",
            "remediation_verification_missing",
            "incident_closure_missing",
            "required_control_disabled",
        ] {
            assert!(
                report.issues.iter().any(|issue| issue.code == code),
                "missing {code}: {:?}",
                report.issues
            );
        }
    }

    #[test]
    fn public_disclosure_requires_advisory_order_safety_and_approval() {
        let mut value = manifest();
        value.disclosures[0].stage = SecurityProgramDisclosureStage::Public;
        value.disclosures[0].advisory_digest = None;
        value.disclosures[0].approval_digest = None;
        value.findings[0].public_safe = false;
        let report = value.audit().expect("audit");
        assert!(!report.valid);
        for code in [
            "disclosure_stage_order_invalid",
            "disclosure_approval_missing",
            "disclosure_advisory_missing",
            "disclosure_finding_not_public_safe",
        ] {
            assert!(
                report.issues.iter().any(|issue| issue.code == code),
                "missing {code}"
            );
        }
    }

    #[test]
    fn security_program_rejects_control_text_and_uppercase_digests() {
        assert!(!bounded_string("allowed\nmethod"));
        assert!(!valid_digest(&"A".repeat(64)));

        let mut value = manifest();
        value.scopes[0].allowed_methods = vec!["fuzz\tunsafe".into()];
        let report = value.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "scope_methods_missing"));
    }

    #[test]
    fn cross_domain_links_must_be_bidirectional_before_readiness() {
        let mut campaign_mismatch = manifest();
        campaign_mismatch.campaigns[0].finding_ids.clear();
        let report = campaign_mismatch.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "finding_campaign_backlink_missing"));
        assert!(!report.finding_audits[0].campaign_valid);

        let mut remediation_mismatch = manifest();
        remediation_mismatch.remediations[0].finding = "another-finding".into();
        let report = remediation_mismatch.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "finding_remediation_backlink_mismatch"));
        assert!(!report.remediation_audits[0].finding_valid);
        assert!(!report.finding_audits[0].remediation_valid);
    }
}
