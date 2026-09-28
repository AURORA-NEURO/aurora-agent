//! Field-level federation sharing gate for preclinical glioma research objects.
//!
//! This gate sits after aggregate compilation and before any consortium exchange.  It binds the
//! recipient scope, quorum, site membership, localization statements, and field policy to every
//! decision.  Human data, raw payloads, revoked sites, and policy-denied fields cannot be promoted
//! by a permissive global default.

use super::federated_release_bundle::{
    FederatedReleaseDisposition, FederatedReleaseError, FederatedResearchObject,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedReleaseSharing1@1";
pub const MAX_FIELDS: usize = 2_048;
pub const MAX_REVOKED_SITES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareFieldRequest {
    pub field_id: String,
    pub source_site_id: String,
    pub aggregate_only: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub policy_permitted: bool,
    pub localization_statement: String,
    pub recipient_scope_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseSharingPolicy {
    pub recipient_scope: String,
    pub required_quorum: usize,
    pub now_epoch: u64,
    pub revoked_site_order: Vec<String>,
    pub denied_field_order: Vec<String>,
    pub permit_redaction: bool,
    pub require_localization: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseSharingRequest {
    pub object: FederatedResearchObject,
    pub fields: Vec<ShareFieldRequest>,
    pub policy: FederatedReleaseSharingPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharingAction {
    Share,
    Redact,
    Deny,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharingFieldDecision {
    pub field_id: String,
    pub source_site_id: String,
    pub action: SharingAction,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharingDisposition {
    Shareable,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedReleaseSharingDecision {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub study_id: String,
    pub object_digest: ContentHash,
    pub recipient_scope: String,
    pub field_decisions: Vec<SharingFieldDecision>,
    pub share_order: Vec<String>,
    pub redact_order: Vec<String>,
    pub deny_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub revoked_site_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub localization_statement: String,
    pub disposition: SharingDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedReleaseSharingError {
    #[error("federated release sharing request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated release sharing decision is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated release sharing digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 512 && !value.contains('\0')
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(decision: &FederatedReleaseSharingDecision) -> serde_json::Value {
    serde_json::json!({
        "feature_id": decision.feature_id,
        "output_schema": decision.output_schema,
        "research_id": decision.research_id,
        "study_id": decision.study_id,
        "object_digest": decision.object_digest,
        "recipient_scope": decision.recipient_scope,
        "field_decisions": decision.field_decisions,
        "share_order": decision.share_order,
        "redact_order": decision.redact_order,
        "deny_order": decision.deny_order,
        "unresolved_order": decision.unresolved_order,
        "revoked_site_order": decision.revoked_site_order,
        "omission_order": decision.omission_order,
        "localization_statement": decision.localization_statement,
        "disposition": decision.disposition,
    })
}

fn validate_request(
    request: &FederatedReleaseSharingRequest,
) -> Result<(), FederatedReleaseSharingError> {
    request
        .object
        .validate()
        .map_err(|error: FederatedReleaseError| {
            FederatedReleaseSharingError::InvalidRequest(error.to_string())
        })?;
    let policy = &request.policy;
    if !identifier(&policy.recipient_scope)
        || policy.required_quorum == 0
        || policy.now_epoch == 0
        || policy.revoked_site_order.len() > MAX_REVOKED_SITES
        || !canonical(&policy.revoked_site_order)
        || !canonical(&policy.denied_field_order)
        || policy
            .revoked_site_order
            .iter()
            .chain(policy.denied_field_order.iter())
            .any(|item| !identifier(item))
        || request.fields.is_empty()
        || request.fields.len() > MAX_FIELDS
    {
        return Err(FederatedReleaseSharingError::InvalidRequest(
            "bounded object, recipient scope, quorum, policy lists, and field requests are required".into(),
        ));
    }
    let mut field_ids = BTreeSet::new();
    for field in &request.fields {
        if !identifier(&field.field_id)
            || !identifier(&field.source_site_id)
            || !text(&field.localization_statement)
            || !canonical(&field.recipient_scope_order)
            || field
                .recipient_scope_order
                .iter()
                .any(|scope| !identifier(scope))
            || !field_ids.insert(field.field_id.clone())
        {
            return Err(FederatedReleaseSharingError::InvalidRequest(
                "field identities, localization, recipient scopes, and uniqueness are required"
                    .into(),
            ));
        }
    }
    Ok(())
}

impl FederatedReleaseSharingDecision {
    pub fn validate(&self) -> Result<(), FederatedReleaseSharingError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.research_id)
            || !identifier(&self.study_id)
            || self.object_digest.as_str().len() != 64
            || !identifier(&self.recipient_scope)
            || !canonical(&self.share_order)
            || !canonical(&self.redact_order)
            || !canonical(&self.deny_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.revoked_site_order)
            || !canonical(&self.omission_order)
            || !text(&self.localization_statement)
            || self
                .field_decisions
                .windows(2)
                .any(|pair| pair[0].field_id >= pair[1].field_id)
            || self.field_decisions.iter().any(|field| {
                !identifier(&field.field_id)
                    || !identifier(&field.source_site_id)
                    || !canonical(&field.reason_order)
                    || field.reason_order.iter().any(|reason| !text(reason))
            })
        {
            return Err(FederatedReleaseSharingError::InvalidOutput(
                "sharing identity, partitions, ordering, localization, or decisions are invalid"
                    .into(),
            ));
        }
        let fields = self
            .field_decisions
            .iter()
            .map(|field| field.field_id.as_str())
            .collect::<BTreeSet<_>>();
        if self
            .share_order
            .iter()
            .chain(self.redact_order.iter())
            .chain(self.deny_order.iter())
            .chain(self.unresolved_order.iter())
            .any(|field| !fields.contains(field.as_str()))
        {
            return Err(FederatedReleaseSharingError::InvalidOutput(
                "sharing action partitions reference unknown fields".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedReleaseSharingError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedReleaseSharingError::InvalidOutput(
                "sharing decision digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Evaluate recipient- and field-level sharing without moving any payload.
pub fn evaluate_glioma_federated_release_sharing(
    request: &FederatedReleaseSharingRequest,
) -> Result<FederatedReleaseSharingDecision, FederatedReleaseSharingError> {
    validate_request(request)?;
    let object_sites = request.object.site_order.iter().collect::<BTreeSet<_>>();
    let revoked = request
        .policy
        .revoked_site_order
        .iter()
        .filter(|site| object_sites.contains(site))
        .cloned()
        .collect::<BTreeSet<_>>();
    let quorum_ok = request.object.site_order.len() >= request.policy.required_quorum
        && request.object.disposition == FederatedReleaseDisposition::ReadyForReview;
    let mut decisions = Vec::new();
    for field in &request.fields {
        let mut reasons = BTreeSet::new();
        let action = if !object_sites.contains(&field.source_site_id) {
            reasons.insert("source-site-not-in-object".into());
            SharingAction::Deny
        } else if revoked.contains(&field.source_site_id) {
            reasons.insert("source-site-revoked".into());
            SharingAction::Deny
        } else if !quorum_ok {
            reasons.insert("quorum-or-object-readiness-failed".into());
            SharingAction::Deny
        } else if field.contains_human_data {
            reasons.insert("human-data-excluded".into());
            SharingAction::Deny
        } else if !field.aggregate_only || field.local_only {
            reasons.insert("aggregate-locality-gate".into());
            SharingAction::Deny
        } else if !field.policy_permitted {
            reasons.insert("field-policy-denied".into());
            SharingAction::Deny
        } else if request
            .policy
            .denied_field_order
            .binary_search(&field.field_id)
            .is_ok()
        {
            reasons.insert("recipient-field-denied".into());
            if request.policy.permit_redaction {
                SharingAction::Redact
            } else {
                SharingAction::Deny
            }
        } else if field
            .recipient_scope_order
            .binary_search(&request.policy.recipient_scope)
            .is_err()
        {
            reasons.insert("recipient-scope-not-authorized".into());
            SharingAction::Deny
        } else if request.policy.require_localization
            && field.localization_statement.trim().is_empty()
        {
            reasons.insert("localization-unresolved".into());
            SharingAction::Unresolved
        } else {
            reasons.insert("all-field-and-consortium-gates-passed".into());
            SharingAction::Share
        };
        decisions.push(SharingFieldDecision {
            field_id: field.field_id.clone(),
            source_site_id: field.source_site_id.clone(),
            action,
            reason_order: reasons.into_iter().collect(),
        });
    }
    decisions.sort_by(|left, right| left.field_id.cmp(&right.field_id));
    let mut share_order = Vec::new();
    let mut redact_order = Vec::new();
    let mut deny_order = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut omissions = BTreeSet::new();
    for decision in &decisions {
        match decision.action {
            SharingAction::Share => share_order.push(decision.field_id.clone()),
            SharingAction::Redact => {
                redact_order.push(decision.field_id.clone());
                omissions.insert(format!("{}:redacted", decision.field_id));
            }
            SharingAction::Deny => {
                deny_order.push(decision.field_id.clone());
                omissions.insert(format!("{}:denied", decision.field_id));
            }
            SharingAction::Unresolved => {
                unresolved_order.push(decision.field_id.clone());
                omissions.insert(format!("{}:unresolved", decision.field_id));
            }
        }
    }
    let disposition = if !deny_order.is_empty() {
        SharingDisposition::Blocked
    } else if !unresolved_order.is_empty() {
        SharingDisposition::Unresolved
    } else if !redact_order.is_empty() {
        SharingDisposition::Partial
    } else {
        SharingDisposition::Shareable
    };
    let mut decision = FederatedReleaseSharingDecision {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.object.research_id.clone(),
        study_id: request.object.study_id.clone(),
        object_digest: request.object.digest.clone(),
        recipient_scope: request.policy.recipient_scope.clone(),
        field_decisions: decisions,
        share_order,
        redact_order,
        deny_order,
        unresolved_order,
        revoked_site_order: revoked.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        localization_statement: request.object.localization_statement.clone(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-sharing-decision"),
    };
    decision.digest = ContentHash::of_value(&digest_input(&decision))
        .map_err(|error| FederatedReleaseSharingError::Digest(error.to_string()))?;
    decision.validate()?;
    Ok(decision)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn request() -> FederatedReleaseSharingRequest {
        let mut object = FederatedResearchObject {
            feature_id: "GAF-GLIOMA-P11-F16".into(),
            output_schema: "GliomaFederatedResearchObject1@1".into(),
            research_id: "research-1".into(),
            study_id: "study-1".into(),
            object_version: "v1".into(),
            site_order: vec!["site-a".into(), "site-b".into()],
            contribution_digest_order: vec![hash("a"), hash("b")],
            omitted_site_order: vec![],
            omission_order: vec![],
            source_count: 8,
            aggregate_uncertainty_milli: 100,
            heterogeneity_milli: 100,
            localization_statement: "raw data remains at origin sites".into(),
            disposition: FederatedReleaseDisposition::ReadyForReview,
            digest: hash("object"),
        };
        object.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": object.feature_id.clone(),
            "output_schema": object.output_schema.clone(),
            "research_id": object.research_id.clone(),
            "study_id": object.study_id.clone(),
            "object_version": object.object_version.clone(),
            "site_order": object.site_order.clone(),
            "contribution_digest_order": object.contribution_digest_order.clone(),
            "omitted_site_order": object.omitted_site_order.clone(),
            "omission_order": object.omission_order.clone(),
            "source_count": object.source_count,
            "aggregate_uncertainty_milli": object.aggregate_uncertainty_milli,
            "heterogeneity_milli": object.heterogeneity_milli,
            "localization_statement": object.localization_statement.clone(),
            "disposition": object.disposition,
        }))
        .unwrap();
        FederatedReleaseSharingRequest {
            object,
            fields: vec![
                ShareFieldRequest {
                    field_id: "effect".into(),
                    source_site_id: "site-a".into(),
                    aggregate_only: true,
                    local_only: false,
                    contains_human_data: false,
                    policy_permitted: true,
                    localization_statement: "site-a local".into(),
                    recipient_scope_order: vec!["consortium".into()],
                },
                ShareFieldRequest {
                    field_id: "uncertainty".into(),
                    source_site_id: "site-b".into(),
                    aggregate_only: true,
                    local_only: false,
                    contains_human_data: false,
                    policy_permitted: true,
                    localization_statement: "site-b local".into(),
                    recipient_scope_order: vec!["consortium".into()],
                },
            ],
            policy: FederatedReleaseSharingPolicy {
                recipient_scope: "consortium".into(),
                required_quorum: 2,
                now_epoch: 100,
                revoked_site_order: vec![],
                denied_field_order: vec![],
                permit_redaction: true,
                require_localization: true,
            },
        }
    }

    #[test]
    fn sharing_gate_admits_aggregate_fields_when_quorum_and_scope_pass() {
        let decision = evaluate_glioma_federated_release_sharing(&request()).unwrap();
        assert_eq!(decision.disposition, SharingDisposition::Shareable);
        assert_eq!(decision.share_order, vec!["effect", "uncertainty"]);
        decision.validate().unwrap();
    }

    #[test]
    fn revoked_site_blocks_even_when_global_policy_is_permissive() {
        let mut request = request();
        request.policy.revoked_site_order = vec!["site-a".into()];
        let decision = evaluate_glioma_federated_release_sharing(&request).unwrap();
        assert_eq!(decision.disposition, SharingDisposition::Blocked);
        assert_eq!(decision.deny_order, vec!["effect"]);
    }

    #[test]
    fn denied_field_can_be_redacted_without_becoming_shareable() {
        let mut request = request();
        request.policy.denied_field_order = vec!["uncertainty".into()];
        let decision = evaluate_glioma_federated_release_sharing(&request).unwrap();
        assert_eq!(decision.disposition, SharingDisposition::Partial);
        assert_eq!(decision.redact_order, vec!["uncertainty"]);
        assert!(decision
            .omission_order
            .contains(&"uncertainty:redacted".into()));
    }
}
