//! Deterministic security and privacy governance evidence over a bounded declaration.
//!
//! This kernel complements the existing threat-model and red-team replay surfaces. It answers a
//! different question: whether a supplied system declaration keeps data assets, permitted flows,
//! identities, threat treatment, review evidence, retention/residency, and controls coherent. It
//! never scans a host, authenticates a person, decrypts data, contacts a vendor, executes a
//! red-team action, or creates legal/privacy authority from a caller assertion.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const SECURITY_PRIVACY_MANIFEST_SCHEMA: &str = "bioprism-security-privacy/0.1";
pub const SECURITY_PRIVACY_AUDIT_SCHEMA: &str = "bioprism-security-privacy-audit/0.1";

const MAX_ASSETS: usize = 4_096;
const MAX_FLOWS: usize = 8_192;
const MAX_IDENTITIES: usize = 8_192;
const MAX_THREATS: usize = 8_192;
const MAX_REVIEWS: usize = 4_096;
const MAX_LIST: usize = 16_384;
const MAX_TEXT_BYTES: usize = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyManifest {
    pub schema: String,
    pub system: SecurityPrivacySystem,
    #[serde(default)]
    pub assets: Vec<SecurityPrivacyAsset>,
    #[serde(default)]
    pub flows: Vec<SecurityPrivacyFlow>,
    #[serde(default)]
    pub identities: Vec<SecurityPrivacyIdentity>,
    #[serde(default)]
    pub threats: Vec<SecurityPrivacyThreat>,
    #[serde(default)]
    pub reviews: Vec<SecurityPrivacyReview>,
    #[serde(default)]
    pub controls: SecurityPrivacyControls,
    #[serde(default)]
    pub policies: SecurityPrivacyPolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacySystem {
    pub id: String,
    pub version: String,
    pub owner: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPrivacyClassification {
    Public,
    Internal,
    Confidential,
    Restricted,
    Regulated,
}

impl SecurityPrivacyClassification {
    fn sensitive(self) -> bool {
        matches!(self, Self::Restricted | Self::Regulated)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyAsset {
    pub id: String,
    pub name: String,
    pub classification: SecurityPrivacyClassification,
    pub owner: String,
    pub purpose: String,
    #[serde(default)]
    pub retention_days: Option<u32>,
    pub residency: String,
    #[serde(default)]
    pub deletion_process: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPrivacyFlowDecision {
    Allow,
    Deny,
    Conditional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyFlow {
    pub id: String,
    pub asset: String,
    pub source: String,
    pub destination: String,
    pub purpose: String,
    #[serde(default)]
    pub legal_basis: Option<String>,
    pub decision: SecurityPrivacyFlowDecision,
    #[serde(default)]
    pub authorization_evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyIdentity {
    pub id: String,
    pub principal: String,
    pub role: String,
    pub authentication: String,
    #[serde(default)]
    pub mfa: bool,
    #[serde(default)]
    pub least_privilege: bool,
    #[serde(default)]
    pub assets: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPrivacyThreatSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl SecurityPrivacyThreatSeverity {
    fn high_or_worse(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPrivacyThreatStatus {
    Mitigated,
    Accepted,
    Unmitigated,
    Unanalysed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyThreat {
    pub id: String,
    pub category: String,
    pub severity: SecurityPrivacyThreatSeverity,
    pub status: SecurityPrivacyThreatStatus,
    #[serde(default)]
    pub control: Option<String>,
    #[serde(default)]
    pub evidence_digest: Option<String>,
    #[serde(default)]
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPrivacyReviewKind {
    PrivacyImpact,
    SecurityAssessment,
    RedTeam,
    AccessReview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPrivacyReviewStatus {
    Draft,
    InReview,
    Complete,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyReview {
    pub id: String,
    pub kind: SecurityPrivacyReviewKind,
    pub scope: String,
    pub reviewer: String,
    pub status: SecurityPrivacyReviewStatus,
    #[serde(default)]
    pub evidence_digest: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyControls {
    #[serde(default)]
    pub access_control: bool,
    #[serde(default)]
    pub encryption_at_rest: bool,
    #[serde(default)]
    pub encryption_in_transit: bool,
    #[serde(default)]
    pub key_rotation: bool,
    #[serde(default)]
    pub audit_logging: bool,
    #[serde(default)]
    pub vulnerability_management: bool,
    #[serde(default)]
    pub backup_restore: bool,
    #[serde(default)]
    pub incident_response: bool,
    #[serde(default)]
    pub vendor_review: bool,
    #[serde(default)]
    pub data_subject_rights: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyPolicies {
    #[serde(default = "default_true")]
    pub require_asset_purpose: bool,
    #[serde(default = "default_true")]
    pub require_retention: bool,
    #[serde(default = "default_true")]
    pub require_flow_authorization: bool,
    #[serde(default = "default_true")]
    pub require_identity_hardening: bool,
    #[serde(default = "default_true")]
    pub require_threat_treatment: bool,
    #[serde(default = "default_true")]
    pub require_reviews: bool,
    #[serde(default = "default_true")]
    pub require_controls: bool,
    #[serde(default = "default_true")]
    pub require_mfa_for_sensitive: bool,
}

impl Default for SecurityPrivacyPolicies {
    fn default() -> Self {
        Self {
            require_asset_purpose: true,
            require_retention: true,
            require_flow_authorization: true,
            require_identity_hardening: true,
            require_threat_treatment: true,
            require_reviews: true,
            require_controls: true,
            require_mfa_for_sensitive: true,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SecurityPrivacyIssueSeverity {
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyIssue {
    pub code: String,
    pub severity: SecurityPrivacyIssueSeverity,
    pub subject: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyAssetAudit {
    pub asset_id: String,
    pub purpose_valid: bool,
    pub retention_valid: bool,
    pub residency_valid: bool,
    pub deletion_valid: bool,
    pub sensitive: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyFlowAudit {
    pub flow_id: String,
    pub asset_valid: bool,
    pub purpose_valid: bool,
    pub legal_basis_present: bool,
    pub authorization_present: bool,
    pub allowed: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyIdentityAudit {
    pub identity_id: String,
    pub assets_valid: bool,
    pub authentication_valid: bool,
    pub mfa: bool,
    pub least_privilege: bool,
    pub sensitive_access: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyThreatAudit {
    pub threat_id: String,
    pub high_or_worse: bool,
    pub treated: bool,
    pub control_present: bool,
    pub evidence_valid: bool,
    pub rationale_present: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyReviewAudit {
    pub review_id: String,
    pub reviewer_independent: bool,
    pub evidence_valid: bool,
    pub current: bool,
    pub complete: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyControlAudit {
    pub control: String,
    pub enabled: bool,
    pub required: bool,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyCounts {
    pub assets: usize,
    pub sensitive_assets: usize,
    pub flows: usize,
    pub allowed_flows: usize,
    pub identities: usize,
    pub hardened_identities: usize,
    pub threats: usize,
    pub high_or_worse_threats: usize,
    pub treated_threats: usize,
    pub reviews: usize,
    pub current_reviews: usize,
    pub controls: usize,
    pub enabled_controls: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityPrivacyAudit {
    pub schema: String,
    pub manifest_schema: String,
    pub digest: String,
    pub valid: bool,
    pub system_id: String,
    pub counts: SecurityPrivacyCounts,
    pub asset_audits: Vec<SecurityPrivacyAssetAudit>,
    pub flow_audits: Vec<SecurityPrivacyFlowAudit>,
    pub identity_audits: Vec<SecurityPrivacyIdentityAudit>,
    pub threat_audits: Vec<SecurityPrivacyThreatAudit>,
    pub review_audits: Vec<SecurityPrivacyReviewAudit>,
    pub control_audits: Vec<SecurityPrivacyControlAudit>,
    pub issues: Vec<SecurityPrivacyIssue>,
    pub guarantees: Vec<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SecurityPrivacyError {
    #[error("cannot canonicalize security/privacy manifest: {0}")]
    Canonical(#[from] bioprism_ids::CanonicalError),
    #[error("cannot serialize security/privacy manifest: {0}")]
    Serialization(String),
}

impl SecurityPrivacyManifest {
    pub fn digest(&self) -> Result<ContentHash, SecurityPrivacyError> {
        let value = serde_json::to_value(self)
            .map_err(|error| SecurityPrivacyError::Serialization(error.to_string()))?;
        Ok(ContentHash::of_value(&value)?)
    }

    pub fn audit(&self) -> Result<SecurityPrivacyAudit, SecurityPrivacyError> {
        let digest = self.digest()?.to_string();
        let mut issues = Vec::new();
        let mut assets = BTreeMap::<String, &SecurityPrivacyAsset>::new();
        let mut flows = BTreeMap::<String, &SecurityPrivacyFlow>::new();
        let mut identities = BTreeMap::<String, &SecurityPrivacyIdentity>::new();
        let mut threats = BTreeMap::<String, &SecurityPrivacyThreat>::new();
        let mut reviews = BTreeMap::<String, &SecurityPrivacyReview>::new();

        bound(&mut issues, "assets", self.assets.len(), MAX_ASSETS);
        bound(&mut issues, "flows", self.flows.len(), MAX_FLOWS);
        bound(
            &mut issues,
            "identities",
            self.identities.len(),
            MAX_IDENTITIES,
        );
        bound(&mut issues, "threats", self.threats.len(), MAX_THREATS);
        bound(&mut issues, "reviews", self.reviews.len(), MAX_REVIEWS);
        if self.schema != SECURITY_PRIVACY_MANIFEST_SCHEMA {
            blocking(
                &mut issues,
                "schema_mismatch",
                "manifest",
                format!(
                    "expected {SECURITY_PRIVACY_MANIFEST_SCHEMA}, got {}",
                    self.schema
                ),
                "regenerate the declaration with the published security/privacy schema",
            );
        }
        for (field, value) in [
            ("system.id", &self.system.id),
            ("system.version", &self.system.version),
            ("system.owner", &self.system.owner),
        ] {
            if !valid_text(value) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    field,
                    format!(
                        "{field} must be non-empty, at most {MAX_TEXT_BYTES} bytes, and contain no control characters"
                    ),
                    "supply bounded visible metadata for the governed system",
                );
            }
        }

        for asset in &self.assets {
            if !insert_unique(&mut assets, &asset.id, "asset", &mut issues) {
                continue;
            }
            assets.insert(asset.id.clone(), asset);
            let sensitive = asset.classification.sensitive();
            for (field, value) in [
                ("id", &asset.id),
                ("name", &asset.name),
                ("owner", &asset.owner),
                ("residency", &asset.residency),
            ] {
                let valid = if field == "id" {
                    valid_identifier(value)
                } else {
                    valid_text(value)
                };
                if !valid {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("asset.{}.{}", asset.id, field),
                        format!(
                            "asset {field} must be canonical, non-empty, at most {MAX_TEXT_BYTES} bytes, and contain no control characters"
                        ),
                        "supply bounded visible asset metadata",
                    );
                }
            }
            if asset.purpose.trim().is_empty() {
                if self.policies.require_asset_purpose {
                    blocking(
                        &mut issues,
                        "asset_purpose_missing",
                        &asset.id,
                        "asset has no declared purpose",
                        "state the permitted purpose before retaining the asset",
                    );
                }
            } else if !valid_text(&asset.purpose) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    &asset.id,
                    "asset purpose contains invalid control or oversized text",
                    "supply bounded visible asset metadata",
                );
            }
            if self.policies.require_retention && sensitive && asset.retention_days.is_none() {
                blocking(
                    &mut issues,
                    "sensitive_retention_missing",
                    &asset.id,
                    "sensitive asset has no retention limit",
                    "declare a bounded retention period and deletion process",
                );
            }
            if self.policies.require_retention
                && sensitive
                && asset
                    .deletion_process
                    .as_deref()
                    .is_none_or(|value| !valid_text(value))
            {
                blocking(
                    &mut issues,
                    "sensitive_deletion_missing",
                    &asset.id,
                    "sensitive asset has no deletion process",
                    "name how retention expiry or erasure requests are fulfilled",
                );
            }
            if let Some(deletion_process) = asset.deletion_process.as_deref() {
                if !valid_text(deletion_process) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("asset.{}.deletion_process", asset.id),
                        "asset deletion process contains invalid control or oversized text",
                        "supply a bounded visible deletion process or omit it",
                    );
                }
            }
        }
        if self.assets.is_empty() {
            blocking(
                &mut issues,
                "assets_missing",
                "assets",
                "the system declares no data assets",
                "inventory data assets before asserting privacy or security posture",
            );
        }

        for flow in &self.flows {
            if !insert_unique(&mut flows, &flow.id, "flow", &mut issues) {
                continue;
            }
            flows.insert(flow.id.clone(), flow);
            if !valid_identifier(&flow.id) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    "flow.id",
                    "flow id must be a canonical, bounded, visible identifier",
                    "supply a stable flow identifier without control characters or surrounding whitespace",
                );
            }
            let asset_valid = assets.contains_key(&flow.asset);
            let purpose_valid = valid_text(&flow.purpose);
            let legal_basis_present = flow
                .legal_basis
                .as_deref()
                .filter(|value| valid_text(value))
                .is_some();
            let authorization_present = flow
                .authorization_evidence
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            if let Some(authorization_evidence) = flow.authorization_evidence.as_deref() {
                if !valid_digest(authorization_evidence) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("flow.{}.authorization_evidence", flow.id),
                        "flow authorization evidence must be a canonical content digest",
                        "supply a lowercase 64-character hexadecimal evidence digest or omit it",
                    );
                }
            }
            if !asset_valid {
                blocking(
                    &mut issues,
                    "flow_asset_missing",
                    &flow.id,
                    format!("flow names undeclared asset {}", flow.asset),
                    "bind every flow to an inventoried asset",
                );
            }
            if !valid_text(&flow.asset)
                || !valid_text(&flow.source)
                || !valid_text(&flow.destination)
                || !purpose_valid
            {
                blocking(
                    &mut issues,
                    "flow_incomplete",
                    &flow.id,
                    "flow asset, source, destination, and purpose must be bounded visible text",
                    "declare who sends what, where, and for which purpose",
                );
            }
            if let Some(legal_basis) = flow.legal_basis.as_deref() {
                if !valid_text(legal_basis) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("flow.{}.legal_basis", flow.id),
                        "flow legal basis contains invalid control or oversized text",
                        "supply bounded visible legal or policy basis text",
                    );
                }
            }
            if flow.decision != SecurityPrivacyFlowDecision::Deny && !legal_basis_present {
                blocking(
                    &mut issues,
                    "flow_legal_basis_missing",
                    &flow.id,
                    "permitted flow has no legal or policy basis",
                    "attach the applicable purpose/legal basis or deny the flow",
                );
            }
            if self.policies.require_flow_authorization
                && flow.decision != SecurityPrivacyFlowDecision::Deny
                && !authorization_present
            {
                blocking(
                    &mut issues,
                    "flow_authorization_missing",
                    &flow.id,
                    "permitted flow has no digest-bound authorization evidence",
                    "bind the flow to an approved authorization record",
                );
            }
        }

        for identity in &self.identities {
            if !insert_unique(&mut identities, &identity.id, "identity", &mut issues) {
                continue;
            }
            identities.insert(identity.id.clone(), identity);
            if !valid_identifier(&identity.id) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    "identity.id",
                    "identity id must be a canonical, bounded, visible identifier",
                    "supply a stable identity identifier without control characters or surrounding whitespace",
                );
            }
            if identity.assets.len() > MAX_LIST {
                bound(
                    &mut issues,
                    "identity.assets",
                    identity.assets.len(),
                    MAX_LIST,
                );
            }
            let assets_valid = identity
                .assets
                .iter()
                .all(|asset| assets.contains_key(asset));
            let authentication_valid = valid_text(&identity.authentication);
            let sensitive_access = identity.assets.iter().any(|asset| {
                assets
                    .get(asset)
                    .map(|a| a.classification.sensitive())
                    .unwrap_or(false)
            });
            if !valid_identifier(&identity.id)
                || !valid_text(&identity.principal)
                || !valid_text(&identity.role)
                || !authentication_valid
            {
                blocking(
                    &mut issues,
                    "identity_incomplete",
                    &identity.id,
                    "identity principal, role, and authentication method are required",
                    "name the principal and its authenticated access path",
                );
            }
            for asset in &identity.assets {
                if !valid_identifier(asset) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("identity.{}.assets", identity.id),
                        "identity asset references must be canonical, bounded, visible identifiers",
                        "bind access only to well-formed inventoried asset identifiers",
                    );
                }
            }
            if !assets_valid {
                blocking(
                    &mut issues,
                    "identity_asset_missing",
                    &identity.id,
                    "identity names an undeclared asset",
                    "bind access only to inventoried assets",
                );
            }
            if self.policies.require_identity_hardening && !identity.least_privilege {
                blocking(
                    &mut issues,
                    "least_privilege_missing",
                    &identity.id,
                    "identity does not declare least-privilege access",
                    "scope the role to the minimum required asset set",
                );
            }
            if self.policies.require_mfa_for_sensitive && sensitive_access && !identity.mfa {
                blocking(
                    &mut issues,
                    "sensitive_mfa_missing",
                    &identity.id,
                    "identity reaches a sensitive asset without MFA",
                    "require multi-factor authentication for sensitive access",
                );
            }
        }

        for threat in &self.threats {
            if !insert_unique(&mut threats, &threat.id, "threat", &mut issues) {
                continue;
            }
            threats.insert(threat.id.clone(), threat);
            if !valid_identifier(&threat.id) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    "threat.id",
                    "threat id must be a canonical, bounded, visible identifier",
                    "supply a stable threat identifier without control characters or surrounding whitespace",
                );
            }
            let high_or_worse = threat.severity.high_or_worse();
            let control_present = threat
                .control
                .as_deref()
                .is_some_and(|value| valid_identifier(value) && valid_control_name(value));
            let evidence_valid = threat
                .evidence_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            if let Some(evidence_digest) = threat.evidence_digest.as_deref() {
                if !valid_digest(evidence_digest) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("threat.{}.evidence_digest", threat.id),
                        "threat evidence must be a canonical content digest",
                        "supply a lowercase 64-character hexadecimal evidence digest or omit it",
                    );
                }
            }
            let rationale_present = threat.rationale.as_deref().is_some_and(valid_text);
            let treated = matches!(
                threat.status,
                SecurityPrivacyThreatStatus::Mitigated | SecurityPrivacyThreatStatus::Accepted
            );
            if !valid_identifier(&threat.id) || !valid_text(&threat.category) {
                blocking(
                    &mut issues,
                    "threat_incomplete",
                    &threat.id,
                    "threat id and category must be canonical bounded visible text",
                    "name the threat family so treatment can be reviewed",
                );
            }
            if let Some(control) = threat.control.as_deref() {
                if !valid_identifier(control) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("threat.{}.control", threat.id),
                        "threat control references must be canonical bounded identifiers",
                        "bind the mitigation to a well-formed declared control",
                    );
                }
            }
            if let Some(rationale) = threat.rationale.as_deref() {
                if !valid_text(rationale) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("threat.{}.rationale", threat.id),
                        "threat rationale contains invalid control or oversized text",
                        "supply bounded visible decision rationale",
                    );
                }
            }
            if self.policies.require_threat_treatment && high_or_worse && !treated {
                blocking(
                    &mut issues,
                    "high_threat_untreated",
                    &threat.id,
                    "high or critical threat is unmitigated or unanalysed",
                    "mitigate it, or record a bounded accepted-risk decision",
                );
            }
            if threat.status == SecurityPrivacyThreatStatus::Mitigated
                && (!control_present || !evidence_valid)
            {
                blocking(
                    &mut issues,
                    "mitigation_evidence_missing",
                    &threat.id,
                    "mitigated threat lacks a named control or evidence digest",
                    "bind the mitigation to a control and content-addressed evidence",
                );
            }
            if threat
                .control
                .as_deref()
                .is_some_and(|control| !valid_control_name(control.trim()))
            {
                blocking(
                    &mut issues,
                    "mitigation_control_unknown",
                    &threat.id,
                    "threat names a control outside the declared control catalogue",
                    "bind the mitigation to one of the explicitly declared security/privacy controls",
                );
            }
            if threat.status == SecurityPrivacyThreatStatus::Accepted
                && (!rationale_present || !evidence_valid)
            {
                blocking(
                    &mut issues,
                    "accepted_risk_record_missing",
                    &threat.id,
                    "accepted risk lacks rationale or evidence",
                    "record the decision basis and its review evidence",
                );
            }
            if threat.status == SecurityPrivacyThreatStatus::Unanalysed {
                warning(
                    &mut issues,
                    "threat_unanalysed",
                    &threat.id,
                    "threat has no analysis result",
                    "perform a bounded analysis before relying on the posture",
                );
            }
        }
        if self.threats.is_empty() && self.policies.require_threat_treatment {
            blocking(
                &mut issues,
                "threats_missing",
                "threats",
                "the system declares no threat treatment rows",
                "record the threat population and its treatment state",
            );
        }

        for review in &self.reviews {
            if !insert_unique(&mut reviews, &review.id, "review", &mut issues) {
                continue;
            }
            reviews.insert(review.id.clone(), review);
            if !valid_identifier(&review.id) {
                blocking(
                    &mut issues,
                    "field_invalid",
                    "review.id",
                    "review id must be a canonical, bounded, visible identifier",
                    "supply a stable review identifier without control characters or surrounding whitespace",
                );
            }
            let reviewer_independent = valid_text(&review.reviewer)
                && !review.reviewer.eq_ignore_ascii_case(&self.system.owner);
            let evidence_valid = review
                .evidence_digest
                .as_deref()
                .map(valid_digest)
                .unwrap_or(false);
            if let Some(evidence_digest) = review.evidence_digest.as_deref() {
                if !valid_digest(evidence_digest) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("review.{}.evidence_digest", review.id),
                        "review evidence must be a canonical content digest",
                        "supply a lowercase 64-character hexadecimal evidence digest or omit it",
                    );
                }
            }
            let current = review.status != SecurityPrivacyReviewStatus::Expired;
            let complete = review.status == SecurityPrivacyReviewStatus::Complete;
            if !valid_identifier(&review.id) || !valid_text(&review.scope) || !reviewer_independent
            {
                blocking(
                    &mut issues,
                    "review_independence_missing",
                    &review.id,
                    "review must name a non-owner reviewer and non-empty scope",
                    "separate authoring ownership from review authority",
                );
            }
            if self.policies.require_reviews && (!complete || !current || !evidence_valid) {
                blocking(
                    &mut issues,
                    "review_evidence_missing",
                    &review.id,
                    "required review is incomplete, expired, or lacks evidence",
                    "complete a current review and bind its evidence digest",
                );
            }
            if review.findings.len() > MAX_LIST {
                bound(
                    &mut issues,
                    "review.findings",
                    review.findings.len(),
                    MAX_LIST,
                );
            }
            if let Some(expires_at) = review.expires_at.as_deref() {
                if !valid_text(expires_at) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("review.{}.expires_at", review.id),
                        "review expiry metadata contains invalid control or oversized text",
                        "supply bounded visible expiry metadata or omit it",
                    );
                }
            }
            for finding in &review.findings {
                if !valid_text(finding) {
                    blocking(
                        &mut issues,
                        "field_invalid",
                        format!("review.{}.findings", review.id),
                        "review findings contain invalid control or oversized text",
                        "supply bounded visible review findings",
                    );
                }
            }
        }
        if self.reviews.is_empty() && self.policies.require_reviews {
            blocking(
                &mut issues,
                "reviews_missing",
                "reviews",
                "the system declares no privacy/security review evidence",
                "attach an independent current review for the governed scope",
            );
        }

        let controls = [
            ("access_control", self.controls.access_control, true),
            (
                "encryption_at_rest",
                self.controls.encryption_at_rest,
                self.assets.iter().any(|a| a.classification.sensitive()),
            ),
            (
                "encryption_in_transit",
                self.controls.encryption_in_transit,
                !self.flows.is_empty(),
            ),
            (
                "key_rotation",
                self.controls.key_rotation,
                self.assets.iter().any(|a| a.classification.sensitive()),
            ),
            ("audit_logging", self.controls.audit_logging, true),
            (
                "vulnerability_management",
                self.controls.vulnerability_management,
                true,
            ),
            (
                "backup_restore",
                self.controls.backup_restore,
                self.assets.iter().any(|a| a.classification.sensitive()),
            ),
            ("incident_response", self.controls.incident_response, true),
            (
                "vendor_review",
                self.controls.vendor_review,
                self.flows.iter().any(|f| {
                    f.destination.to_ascii_lowercase().contains("vendor")
                        || f.destination.to_ascii_lowercase().contains("external")
                }),
            ),
            (
                "data_subject_rights",
                self.controls.data_subject_rights,
                self.assets
                    .iter()
                    .any(|a| matches!(a.classification, SecurityPrivacyClassification::Regulated)),
            ),
        ];
        for (name, enabled, required) in controls {
            if self.policies.require_controls && required && !enabled {
                blocking(
                    &mut issues,
                    "required_control_disabled",
                    name,
                    format!("required control {name} is disabled"),
                    "enable the control or narrow the declared governed scope",
                );
            }
        }

        let asset_audits = self
            .assets
            .iter()
            .map(|asset| {
                let sensitive = asset.classification.sensitive();
                let purpose_valid = valid_text(&asset.purpose);
                let retention_valid = !sensitive || asset.retention_days.is_some();
                let residency_valid = valid_text(&asset.residency);
                let deletion_valid =
                    !sensitive || asset.deletion_process.as_deref().is_some_and(valid_text);
                SecurityPrivacyAssetAudit {
                    asset_id: asset.id.clone(),
                    purpose_valid,
                    retention_valid,
                    residency_valid,
                    deletion_valid,
                    sensitive,
                    ready: purpose_valid && retention_valid && residency_valid && deletion_valid,
                }
            })
            .collect::<Vec<_>>();
        let flow_audits = self
            .flows
            .iter()
            .map(|flow| {
                let asset_valid = assets.contains_key(&flow.asset);
                let purpose_valid = valid_text(&flow.purpose);
                let legal_basis_present = flow.legal_basis.as_deref().is_some_and(valid_text);
                let authorization_present = flow
                    .authorization_evidence
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                let allowed = flow.decision != SecurityPrivacyFlowDecision::Deny;
                SecurityPrivacyFlowAudit {
                    flow_id: flow.id.clone(),
                    asset_valid,
                    purpose_valid,
                    legal_basis_present,
                    authorization_present,
                    allowed,
                    ready: asset_valid
                        && purpose_valid
                        && (!allowed || (legal_basis_present && authorization_present)),
                }
            })
            .collect::<Vec<_>>();
        let identity_audits = self
            .identities
            .iter()
            .map(|identity| {
                let assets_valid = identity
                    .assets
                    .iter()
                    .all(|asset| assets.contains_key(asset));
                let authentication_valid = valid_text(&identity.authentication);
                let sensitive_access = identity.assets.iter().any(|asset| {
                    assets
                        .get(asset)
                        .map(|a| a.classification.sensitive())
                        .unwrap_or(false)
                });
                let ready = assets_valid
                    && authentication_valid
                    && identity.least_privilege
                    && (!sensitive_access || identity.mfa);
                SecurityPrivacyIdentityAudit {
                    identity_id: identity.id.clone(),
                    assets_valid,
                    authentication_valid,
                    mfa: identity.mfa,
                    least_privilege: identity.least_privilege,
                    sensitive_access,
                    ready,
                }
            })
            .collect::<Vec<_>>();
        let threat_audits = self
            .threats
            .iter()
            .map(|threat| {
                let high_or_worse = threat.severity.high_or_worse();
                let treated = matches!(
                    threat.status,
                    SecurityPrivacyThreatStatus::Mitigated | SecurityPrivacyThreatStatus::Accepted
                );
                let control_present = threat
                    .control
                    .as_deref()
                    .is_some_and(|value| valid_identifier(value) && valid_control_name(value));
                let evidence_valid = threat
                    .evidence_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                let rationale_present = threat.rationale.as_deref().is_some_and(valid_text);
                let ready = (!high_or_worse || treated)
                    && match threat.status {
                        SecurityPrivacyThreatStatus::Mitigated => control_present && evidence_valid,
                        SecurityPrivacyThreatStatus::Accepted => {
                            rationale_present && evidence_valid
                        }
                        _ => false,
                    };
                SecurityPrivacyThreatAudit {
                    threat_id: threat.id.clone(),
                    high_or_worse,
                    treated,
                    control_present,
                    evidence_valid,
                    rationale_present,
                    ready,
                }
            })
            .collect::<Vec<_>>();
        let review_audits = self
            .reviews
            .iter()
            .map(|review| {
                let reviewer_independent = valid_text(&review.reviewer)
                    && !review.reviewer.eq_ignore_ascii_case(&self.system.owner);
                let evidence_valid = review
                    .evidence_digest
                    .as_deref()
                    .map(valid_digest)
                    .unwrap_or(false);
                let current = review.status != SecurityPrivacyReviewStatus::Expired;
                let complete = review.status == SecurityPrivacyReviewStatus::Complete;
                SecurityPrivacyReviewAudit {
                    review_id: review.id.clone(),
                    reviewer_independent,
                    evidence_valid,
                    current,
                    complete,
                    ready: reviewer_independent && evidence_valid && current && complete,
                }
            })
            .collect::<Vec<_>>();
        let control_audits = controls_from(&self.controls, &self.assets, &self.flows);
        let enabled_controls = control_audits
            .iter()
            .filter(|control| control.enabled)
            .count();
        let high_or_worse_threats = self
            .threats
            .iter()
            .filter(|threat| threat.severity.high_or_worse())
            .count();
        let treated_threats = self
            .threats
            .iter()
            .filter(|threat| {
                matches!(
                    threat.status,
                    SecurityPrivacyThreatStatus::Mitigated | SecurityPrivacyThreatStatus::Accepted
                )
            })
            .count();
        let current_reviews = self
            .reviews
            .iter()
            .filter(|review| review.status != SecurityPrivacyReviewStatus::Expired)
            .count();
        let counts = SecurityPrivacyCounts {
            assets: self.assets.len(),
            sensitive_assets: self
                .assets
                .iter()
                .filter(|asset| asset.classification.sensitive())
                .count(),
            flows: self.flows.len(),
            allowed_flows: self
                .flows
                .iter()
                .filter(|flow| flow.decision != SecurityPrivacyFlowDecision::Deny)
                .count(),
            identities: self.identities.len(),
            hardened_identities: identity_audits
                .iter()
                .filter(|identity| identity.ready)
                .count(),
            threats: self.threats.len(),
            high_or_worse_threats,
            treated_threats,
            reviews: self.reviews.len(),
            current_reviews,
            controls: control_audits.len(),
            enabled_controls,
        };
        let valid = !issues
            .iter()
            .any(|issue| issue.severity == SecurityPrivacyIssueSeverity::Blocking);
        Ok(SecurityPrivacyAudit {
            schema: SECURITY_PRIVACY_AUDIT_SCHEMA.into(),
            manifest_schema: self.schema.clone(),
            digest,
            valid,
            system_id: self.system.id.clone(),
            counts,
            asset_audits,
            flow_audits,
            identity_audits,
            threat_audits,
            review_audits,
            control_audits,
            issues,
            guarantees: vec![
                "data assets, flows, identities, threat treatment, reviews, and controls remain separate evidence layers".into(),
                "sensitive retention, flow authorization, identity hardening, and review evidence remain explicit".into(),
                "blocking posture is derived from named issues rather than caller-supplied readiness booleans".into(),
            ],
            limitations: vec![
                "the audit does not scan infrastructure, authenticate identities, or verify a legal basis".into(),
                "the audit does not execute red-team actions, test encryption, erase data, or contact vendors".into(),
                "all declarations, evidence digests, dates, and control states are caller-supplied".into(),
            ],
        })
    }
}

fn controls_from(
    controls: &SecurityPrivacyControls,
    assets: &[SecurityPrivacyAsset],
    flows: &[SecurityPrivacyFlow],
) -> Vec<SecurityPrivacyControlAudit> {
    let rows = [
        ("access_control", controls.access_control, true),
        (
            "encryption_at_rest",
            controls.encryption_at_rest,
            assets.iter().any(|asset| asset.classification.sensitive()),
        ),
        (
            "encryption_in_transit",
            controls.encryption_in_transit,
            !flows.is_empty(),
        ),
        (
            "key_rotation",
            controls.key_rotation,
            assets.iter().any(|asset| asset.classification.sensitive()),
        ),
        ("audit_logging", controls.audit_logging, true),
        (
            "vulnerability_management",
            controls.vulnerability_management,
            true,
        ),
        (
            "backup_restore",
            controls.backup_restore,
            assets.iter().any(|asset| asset.classification.sensitive()),
        ),
        ("incident_response", controls.incident_response, true),
        (
            "vendor_review",
            controls.vendor_review,
            flows.iter().any(|flow| {
                flow.destination.to_ascii_lowercase().contains("vendor")
                    || flow.destination.to_ascii_lowercase().contains("external")
            }),
        ),
        (
            "data_subject_rights",
            controls.data_subject_rights,
            assets.iter().any(|asset| {
                matches!(
                    asset.classification,
                    SecurityPrivacyClassification::Regulated
                )
            }),
        ),
    ];
    rows.into_iter()
        .map(|(control, enabled, required)| SecurityPrivacyControlAudit {
            control: control.into(),
            enabled,
            required,
            ready: !required || enabled,
        })
        .collect()
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

fn valid_control_name(value: &str) -> bool {
    matches!(
        value,
        "access_control"
            | "encryption_at_rest"
            | "encryption_in_transit"
            | "key_rotation"
            | "audit_logging"
            | "vulnerability_management"
            | "backup_restore"
            | "incident_response"
            | "vendor_review"
            | "data_subject_rights"
    )
}

fn insert_unique<T>(
    map: &mut BTreeMap<String, &T>,
    id: &str,
    kind: &str,
    issues: &mut Vec<SecurityPrivacyIssue>,
) -> bool {
    if map
        .keys()
        .any(|existing| existing == id || existing.eq_ignore_ascii_case(id))
    {
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

fn bound(issues: &mut Vec<SecurityPrivacyIssue>, field: &str, actual: usize, maximum: usize) {
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
    issues: &mut Vec<SecurityPrivacyIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(SecurityPrivacyIssue {
        code: code.into(),
        severity: SecurityPrivacyIssueSeverity::Blocking,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

fn warning(
    issues: &mut Vec<SecurityPrivacyIssue>,
    code: &str,
    subject: impl Into<String>,
    detail: impl Into<String>,
    remediation: impl Into<String>,
) {
    issues.push(SecurityPrivacyIssue {
        code: code.into(),
        severity: SecurityPrivacyIssueSeverity::Warning,
        subject: subject.into(),
        detail: detail.into(),
        remediation: remediation.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> SecurityPrivacyManifest {
        SecurityPrivacyManifest {
            schema: SECURITY_PRIVACY_MANIFEST_SCHEMA.into(),
            system: SecurityPrivacySystem {
                id: "aurora-api".into(),
                version: "0.1.0".into(),
                owner: "platform".into(),
            },
            assets: vec![SecurityPrivacyAsset {
                id: "patient-records".into(),
                name: "records".into(),
                classification: SecurityPrivacyClassification::Regulated,
                owner: "privacy".into(),
                purpose: "care research".into(),
                retention_days: Some(365),
                residency: "us".into(),
                deletion_process: Some("erase workflow".into()),
            }],
            flows: vec![SecurityPrivacyFlow {
                id: "api-to-vendor".into(),
                asset: "patient-records".into(),
                source: "api".into(),
                destination: "approved-vendor".into(),
                purpose: "care research".into(),
                legal_basis: Some("consent".into()),
                decision: SecurityPrivacyFlowDecision::Allow,
                authorization_evidence: Some("a".repeat(64)),
            }],
            identities: vec![SecurityPrivacyIdentity {
                id: "researcher".into(),
                principal: "team".into(),
                role: "research".into(),
                authentication: "oidc".into(),
                mfa: true,
                least_privilege: true,
                assets: vec!["patient-records".into()],
            }],
            threats: vec![SecurityPrivacyThreat {
                id: "exfiltration".into(),
                category: "data-exfiltration".into(),
                severity: SecurityPrivacyThreatSeverity::High,
                status: SecurityPrivacyThreatStatus::Mitigated,
                control: Some("vulnerability_management".into()),
                evidence_digest: Some("a".repeat(64)),
                rationale: None,
            }],
            reviews: vec![SecurityPrivacyReview {
                id: "pia-1".into(),
                kind: SecurityPrivacyReviewKind::PrivacyImpact,
                scope: "patient-records".into(),
                reviewer: "independent-reviewer".into(),
                status: SecurityPrivacyReviewStatus::Complete,
                evidence_digest: Some("a".repeat(64)),
                expires_at: Some("2027-01-01".into()),
                findings: vec!["none".into()],
            }],
            controls: SecurityPrivacyControls {
                access_control: true,
                encryption_at_rest: true,
                encryption_in_transit: true,
                key_rotation: true,
                audit_logging: true,
                vulnerability_management: true,
                backup_restore: true,
                incident_response: true,
                vendor_review: true,
                data_subject_rights: true,
            },
            policies: SecurityPrivacyPolicies::default(),
        }
    }

    #[test]
    fn valid_manifest_preserves_data_flow_identity_threat_review_and_control_layers() {
        let report = manifest().audit().expect("audit");
        assert!(report.valid);
        assert_eq!(report.counts.sensitive_assets, 1);
        assert!(report.flow_audits[0].authorization_present);
        assert!(report.identity_audits[0].ready);
        assert!(report.threat_audits[0].evidence_valid);
        assert!(report.review_audits[0].complete);
        assert_eq!(report.counts.enabled_controls, 10);
    }

    #[test]
    fn missing_sensitive_governance_evidence_is_blocking_and_layered() {
        let mut value = manifest();
        value.assets[0].retention_days = None;
        value.flows[0].authorization_evidence = None;
        value.identities[0].mfa = false;
        value.threats[0].evidence_digest = None;
        value.reviews[0].status = SecurityPrivacyReviewStatus::Expired;
        value.controls.encryption_at_rest = false;
        let report = value.audit().expect("audit");
        assert!(!report.valid);
        for code in [
            "sensitive_retention_missing",
            "flow_authorization_missing",
            "sensitive_mfa_missing",
            "mitigation_evidence_missing",
            "review_evidence_missing",
            "required_control_disabled",
        ] {
            assert!(
                report.issues.iter().any(|issue| issue.code == code),
                "missing {code}"
            );
        }
    }

    #[test]
    fn accepted_risk_requires_a_bounded_decision_record() {
        let mut value = manifest();
        value.threats[0].status = SecurityPrivacyThreatStatus::Accepted;
        value.threats[0].rationale = None;
        let report = value.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "accepted_risk_record_missing"));
    }

    #[test]
    fn mitigated_threats_must_name_a_declared_control() {
        let mut value = manifest();
        value.threats[0].control = Some("invented_control".into());

        let report = value.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "mitigation_control_unknown"));
        assert!(!report.threat_audits[0].control_present);
        assert!(!report.threat_audits[0].ready);
    }

    #[test]
    fn security_privacy_rejects_noncanonical_evidence_and_control_metadata() {
        let mut value = manifest();
        value.flows[0].authorization_evidence = Some("A".repeat(64));
        value.threats[0].evidence_digest = Some("A".repeat(64));
        value.reviews[0].evidence_digest = Some("A".repeat(64));
        value.identities[0].role = "research\noperator".into();
        value.reviews[0].findings = vec!["f".repeat(MAX_TEXT_BYTES + 1)];

        let report = value.audit().expect("audit");
        assert!(!report.valid);
        for code in [
            "flow_authorization_missing",
            "mitigation_evidence_missing",
            "review_evidence_missing",
            "field_invalid",
        ] {
            assert!(
                report.issues.iter().any(|issue| issue.code == code),
                "missing {code}"
            );
        }
        for subject in [
            "flow.api-to-vendor.authorization_evidence",
            "threat.exfiltration.evidence_digest",
            "review.pia-1.evidence_digest",
        ] {
            assert!(
                report
                    .issues
                    .iter()
                    .any(|issue| issue.code == "field_invalid" && issue.subject == subject),
                "missing invalid-evidence issue for {subject}"
            );
        }
        assert!(!valid_digest(&"A".repeat(64)));
        assert!(valid_digest(&"a".repeat(64)));
    }

    #[test]
    fn security_privacy_rejects_case_colliding_identifiers() {
        let mut value = manifest();
        value.assets.push(SecurityPrivacyAsset {
            id: "PATIENT-RECORDS".into(),
            name: "duplicate records".into(),
            classification: SecurityPrivacyClassification::Internal,
            owner: "privacy".into(),
            purpose: "duplicate test row".into(),
            retention_days: Some(30),
            residency: "us".into(),
            deletion_process: Some("erase workflow".into()),
        });

        let report = value.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "duplicate_id"));
    }

    #[test]
    fn security_privacy_rejects_padded_text_and_case_alias_owner_reviews() {
        let mut value = manifest();
        value.assets[0].purpose = " care research".into();
        value.reviews[0].reviewer = "PLATFORM".into();

        let report = value.audit().expect("audit");
        assert!(!report.valid);
        assert!(report
            .issues
            .iter()
            .any(|issue| { issue.code == "field_invalid" && issue.subject == "patient-records" }));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.code == "review_independence_missing"));
    }
}
