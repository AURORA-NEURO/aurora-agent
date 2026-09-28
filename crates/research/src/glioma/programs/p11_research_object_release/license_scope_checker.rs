//! Transitive license, locality, embargo, and audience shareability evaluation for releases.
//!
//! The checker evaluates a caller-supplied dependency graph and field declarations before any
//! export.  It never interprets a license by guessing, never lets a permissive parent override a
//! restrictive child, and keeps redaction, denial, and unresolved-rights states separate.  Raw
//! data, human data, credentials, and clinical decisions remain outside this product boundary.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaReleaseShareabilityDecision1@1";
pub const MAX_DEPENDENCIES: usize = 2048;
pub const MAX_FIELDS_PER_DEPENDENCY: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldClassification {
    PublicMetadata,
    AggregateResult,
    RawExperimentalData,
    DirectIdentifier,
    Secret,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareableField {
    pub field_id: String,
    pub classification: FieldClassification,
    pub requested_export: bool,
    pub source_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseDependency {
    pub artifact_id: String,
    pub dependency_order: Vec<String>,
    pub license_id: Option<String>,
    pub fields: Vec<ShareableField>,
    pub local_only: bool,
    pub embargo_until_epoch: Option<u64>,
    pub rights_confirmed: bool,
    pub contains_human_data: bool,
    pub intended_audience: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseScopePolicy {
    pub allowed_license_order: Vec<String>,
    pub forbidden_license_order: Vec<String>,
    pub audience: String,
    pub now_epoch: u64,
    pub permit_aggregate_export: bool,
    pub permit_local_only_export: bool,
    pub permit_human_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseShareabilityRequest {
    pub candidate_manifest_digest: ContentHash,
    pub root_order: Vec<String>,
    pub dependencies: Vec<LicenseDependency>,
    pub policy: LicenseScopePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareabilityDecision {
    Allow,
    Redact,
    Deny,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldShareabilityDecision {
    pub artifact_id: String,
    pub field_id: String,
    pub classification: FieldClassification,
    pub decision: ShareabilityDecision,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseShareabilityDecision {
    pub feature_id: String,
    pub output_schema: String,
    pub candidate_manifest_digest: ContentHash,
    pub audience: String,
    pub traversal_order: Vec<String>,
    pub field_decisions: Vec<FieldShareabilityDecision>,
    pub allow_order: Vec<String>,
    pub redact_order: Vec<String>,
    pub deny_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocking_order: Vec<String>,
    pub disposition: ShareabilityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareabilityDisposition {
    Shareable,
    RedactionRequired,
    Blocked,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LicenseScopeError {
    #[error("license scope request is invalid: {0}")]
    InvalidRequest(String),
    #[error("license scope output is invalid: {0}")]
    InvalidOutput(String),
    #[error("license scope digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.contains('\0')
}

fn digest_input(output: &ReleaseShareabilityDecision) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "candidate_manifest_digest": output.candidate_manifest_digest,
        "audience": output.audience,
        "traversal_order": output.traversal_order,
        "field_decisions": output.field_decisions,
        "allow_order": output.allow_order,
        "redact_order": output.redact_order,
        "deny_order": output.deny_order,
        "unresolved_order": output.unresolved_order,
        "blocking_order": output.blocking_order,
        "disposition": output.disposition,
    })
}

impl ReleaseShareabilityDecision {
    pub fn validate(&self) -> Result<(), LicenseScopeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.candidate_manifest_digest.as_str().len() != 64
            || !valid_text(&self.audience)
            || !canonical(&self.traversal_order)
            || !canonical(&self.allow_order)
            || !canonical(&self.redact_order)
            || !canonical(&self.deny_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.blocking_order)
            || self
                .allow_order
                .iter()
                .chain(self.redact_order.iter())
                .chain(self.deny_order.iter())
                .chain(self.unresolved_order.iter())
                .any(|id| !valid_text(id))
        {
            return Err(LicenseScopeError::InvalidOutput(
                "shareability identity, audience, ordering, or partition is invalid".into(),
            ));
        }
        for field in &self.field_decisions {
            if !valid_identifier(&field.artifact_id)
                || !valid_identifier(&field.field_id)
                || !canonical(&field.reasons)
                || field.reasons.is_empty()
            {
                return Err(LicenseScopeError::InvalidOutput(
                    "field decisions must retain canonical reason codes".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LicenseScopeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(LicenseScopeError::InvalidOutput(
                "shareability digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &ReleaseShareabilityRequest) -> Result<(), LicenseScopeError> {
    if request.candidate_manifest_digest.as_str().len() != 64
        || request.root_order.is_empty()
        || !canonical(&request.root_order)
        || request.dependencies.is_empty()
        || request.dependencies.len() > MAX_DEPENDENCIES
        || !valid_text(&request.policy.audience)
        || request
            .policy
            .allowed_license_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .policy
            .forbidden_license_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(LicenseScopeError::InvalidRequest(
            "manifest, canonical roots/licenses, bounded dependencies, and audience policy are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let root_set = request.root_order.iter().collect::<BTreeSet<_>>();
    for root in &request.root_order {
        if !valid_identifier(root) {
            return Err(LicenseScopeError::InvalidRequest(
                "root identifiers are invalid".into(),
            ));
        }
    }
    for dependency in &request.dependencies {
        if !valid_identifier(&dependency.artifact_id)
            || !ids.insert(dependency.artifact_id.clone())
            || dependency.fields.len() > MAX_FIELDS_PER_DEPENDENCY
            || !valid_text(&dependency.intended_audience)
            || dependency.fields.is_empty()
            || dependency
                .dependency_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(LicenseScopeError::InvalidRequest(
                "dependency identity, bounded fields, audience, and canonical edges are required"
                    .into(),
            ));
        }
        let mut field_ids = BTreeSet::new();
        for field in &dependency.fields {
            if !valid_identifier(&field.field_id)
                || !field_ids.insert(field.field_id.clone())
                || field.source_digest.as_str().len() != 64
            {
                return Err(LicenseScopeError::InvalidRequest(
                    "field identities and source digests must be unique and valid".into(),
                ));
            }
        }
        if dependency
            .dependency_order
            .iter()
            .any(|edge| !valid_identifier(edge))
        {
            return Err(LicenseScopeError::InvalidRequest(
                "dependency edges are invalid".into(),
            ));
        }
    }
    if root_set.iter().any(|root| !ids.contains(*root)) {
        return Err(LicenseScopeError::InvalidRequest(
            "every release root must have a dependency declaration".into(),
        ));
    }
    Ok(())
}

/// Evaluate transitive release rights and produce a field-level export/redaction plan.
pub fn evaluate_glioma_release_shareability(
    request: &ReleaseShareabilityRequest,
) -> Result<ReleaseShareabilityDecision, LicenseScopeError> {
    validate_request(request)?;
    let dependencies = request
        .dependencies
        .iter()
        .map(|dependency| (dependency.artifact_id.clone(), dependency))
        .collect::<BTreeMap<_, _>>();
    let allowed = request
        .policy
        .allowed_license_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let forbidden = request
        .policy
        .forbidden_license_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut queue = VecDeque::from(request.root_order.clone());
    let mut visited = BTreeSet::new();
    let mut traversal_order = Vec::new();
    let mut field_decisions = Vec::new();
    let mut blocking = BTreeSet::new();
    while let Some(artifact_id) = queue.pop_front() {
        if !visited.insert(artifact_id.clone()) {
            continue;
        }
        let Some(dependency) = dependencies.get(&artifact_id) else {
            blocking.insert(format!("missing-dependency:{artifact_id}"));
            continue;
        };
        traversal_order.push(artifact_id.clone());
        for edge in &dependency.dependency_order {
            queue.push_back(edge.clone());
        }
        let mut artifact_reasons = Vec::new();
        if dependency.intended_audience != request.policy.audience {
            artifact_reasons.push("audience-scope-mismatch".into());
        }
        if dependency.local_only && !request.policy.permit_local_only_export {
            artifact_reasons.push("local-only-export-denied".into());
        }
        if dependency.contains_human_data && !request.policy.permit_human_data {
            artifact_reasons.push("human-data-out-of-scope".into());
        }
        if let Some(until) = dependency.embargo_until_epoch {
            if until > request.policy.now_epoch {
                artifact_reasons.push(format!("embargo-active-until:{until}"));
            }
        }
        match dependency.license_id.as_deref() {
            Some(license) if forbidden.contains(license) => {
                artifact_reasons.push(format!("forbidden-license:{license}"))
            }
            Some(license) if !allowed.contains(license) => {
                artifact_reasons.push(format!("license-not-allowlisted:{license}"))
            }
            None => artifact_reasons.push("license-unknown".into()),
            _ => {}
        }
        if !dependency.rights_confirmed {
            artifact_reasons.push("rights-unconfirmed".into());
        }
        artifact_reasons.sort();
        for field in &dependency.fields {
            let mut reasons = artifact_reasons.clone();
            let decision = match field.classification {
                FieldClassification::DirectIdentifier | FieldClassification::Secret => {
                    reasons.push("protected-field".into());
                    ShareabilityDecision::Deny
                }
                FieldClassification::RawExperimentalData => {
                    reasons.push("raw-data-local-by-default".into());
                    ShareabilityDecision::Redact
                }
                FieldClassification::AggregateResult if !request.policy.permit_aggregate_export => {
                    reasons.push("aggregate-export-disabled".into());
                    ShareabilityDecision::Deny
                }
                _ if !field.requested_export => {
                    reasons.push("field-not-requested".into());
                    ShareabilityDecision::Redact
                }
                _ if artifact_reasons.iter().any(|reason| {
                    reason.starts_with("license-") || reason == "rights-unconfirmed"
                }) =>
                {
                    ShareabilityDecision::Unresolved
                }
                _ if !artifact_reasons.is_empty() => ShareabilityDecision::Deny,
                _ => ShareabilityDecision::Allow,
            };
            reasons.sort();
            reasons.dedup();
            if reasons.is_empty() {
                reasons.push("rights-confirmed-and-policy-allowed".into());
            }
            field_decisions.push(FieldShareabilityDecision {
                artifact_id: artifact_id.clone(),
                field_id: field.field_id.clone(),
                classification: field.classification,
                decision,
                reasons,
            });
        }
        if !artifact_reasons.is_empty() {
            blocking.extend(
                artifact_reasons
                    .into_iter()
                    .map(|reason| format!("{artifact_id}:{reason}")),
            );
        }
    }
    traversal_order.sort();
    field_decisions.sort_by(|left, right| {
        (left.artifact_id.clone(), left.field_id.clone())
            .cmp(&(right.artifact_id.clone(), right.field_id.clone()))
    });
    let mut allow_order = Vec::new();
    let mut redact_order = Vec::new();
    let mut deny_order = Vec::new();
    let mut unresolved_order = Vec::new();
    for field in &field_decisions {
        let id = format!("{}:{}", field.artifact_id, field.field_id);
        match field.decision {
            ShareabilityDecision::Allow => allow_order.push(id),
            ShareabilityDecision::Redact => redact_order.push(id),
            ShareabilityDecision::Deny => deny_order.push(id),
            ShareabilityDecision::Unresolved => unresolved_order.push(id),
        }
    }
    let disposition =
        if !deny_order.is_empty() || !unresolved_order.is_empty() || !blocking.is_empty() {
            ShareabilityDisposition::Blocked
        } else if !redact_order.is_empty() {
            ShareabilityDisposition::RedactionRequired
        } else {
            ShareabilityDisposition::Shareable
        };
    let mut output = ReleaseShareabilityDecision {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        candidate_manifest_digest: request.candidate_manifest_digest.clone(),
        audience: request.policy.audience.clone(),
        traversal_order,
        field_decisions,
        allow_order,
        redact_order,
        deny_order,
        unresolved_order,
        blocking_order: blocking.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-release-shareability"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| LicenseScopeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn field(id: &str, classification: FieldClassification) -> ShareableField {
        ShareableField {
            field_id: id.into(),
            classification,
            requested_export: true,
            source_digest: hash(id),
        }
    }

    fn request() -> ReleaseShareabilityRequest {
        ReleaseShareabilityRequest {
            candidate_manifest_digest: hash("manifest"),
            root_order: vec!["root".into()],
            dependencies: vec![
                LicenseDependency {
                    artifact_id: "root".into(),
                    dependency_order: vec!["upstream".into()],
                    license_id: Some("CC-BY-4.0".into()),
                    fields: vec![field("summary", FieldClassification::AggregateResult)],
                    local_only: false,
                    embargo_until_epoch: None,
                    rights_confirmed: true,
                    contains_human_data: false,
                    intended_audience: "consortium".into(),
                },
                LicenseDependency {
                    artifact_id: "upstream".into(),
                    dependency_order: Vec::new(),
                    license_id: Some("MIT".into()),
                    fields: vec![field("methods", FieldClassification::PublicMetadata)],
                    local_only: false,
                    embargo_until_epoch: None,
                    rights_confirmed: true,
                    contains_human_data: false,
                    intended_audience: "consortium".into(),
                },
            ],
            policy: LicenseScopePolicy {
                allowed_license_order: vec!["CC-BY-4.0".into(), "MIT".into()],
                forbidden_license_order: vec!["PROPRIETARY".into()],
                audience: "consortium".into(),
                now_epoch: 20260923,
                permit_aggregate_export: true,
                permit_local_only_export: false,
                permit_human_data: false,
            },
        }
    }

    #[test]
    fn transitive_allowlist_produces_shareable_decision() {
        let output = evaluate_glioma_release_shareability(&request()).unwrap();
        assert_eq!(output.disposition, ShareabilityDisposition::Shareable);
        assert_eq!(output.traversal_order, vec!["root", "upstream"]);
        assert_eq!(output.allow_order.len(), 2);
        output.validate().unwrap();
    }

    #[test]
    fn unknown_rights_block_without_guessing() {
        let mut request = request();
        request.dependencies[1].license_id = None;
        let output = evaluate_glioma_release_shareability(&request).unwrap();
        assert_eq!(output.disposition, ShareabilityDisposition::Blocked);
        assert!(output
            .unresolved_order
            .iter()
            .any(|id| id.ends_with(":methods")));
    }

    #[test]
    fn raw_and_protected_fields_are_not_exportable() {
        let mut request = request();
        request.dependencies[0]
            .fields
            .push(field("raw", FieldClassification::RawExperimentalData));
        request.dependencies[0]
            .fields
            .push(field("secret", FieldClassification::Secret));
        let output = evaluate_glioma_release_shareability(&request).unwrap();
        assert_eq!(output.disposition, ShareabilityDisposition::Blocked);
        assert!(output.redact_order.iter().any(|id| id.ends_with(":raw")));
        assert!(output.deny_order.iter().any(|id| id.ends_with(":secret")));
    }

    #[test]
    fn active_embargo_and_audience_mismatch_block_transitive_export() {
        let mut request = request();
        request.dependencies[0].embargo_until_epoch = Some(20270000);
        request.dependencies[1].intended_audience = "local".into();
        let output = evaluate_glioma_release_shareability(&request).unwrap();
        assert_eq!(output.disposition, ShareabilityDisposition::Blocked);
        assert!(output
            .blocking_order
            .iter()
            .any(|reason| reason.contains("embargo-active")));
        assert!(output
            .blocking_order
            .iter()
            .any(|reason| reason.contains("audience-scope")));
    }

    #[test]
    fn missing_dependency_is_explicit() {
        let mut request = request();
        request.dependencies[0].dependency_order = vec!["missing".into()];
        let output = evaluate_glioma_release_shareability(&request).unwrap();
        assert!(output
            .blocking_order
            .iter()
            .any(|reason| reason == "missing-dependency:missing"));
    }
}
