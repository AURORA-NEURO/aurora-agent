//! Signed, bounded decision-context capsules for federated preclinical glioma research.
//!
//! A site exports a context summary, evidence-coverage identifiers, omissions, uncertainty, and
//! explicitly permitted downstream action identifiers. It never exports raw study payloads or
//! silently expands the receiving site's authority.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaFederationDecisionContext1@1";
pub const MAX_ITEMS: usize = 512;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalDecisionContextCapsule {
    pub capsule_id: String,
    pub source_site_id: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub evidence_coverage_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub downstream_action_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub generated_tick: u64,
    pub expires_at_tick: u64,
    pub local_only: bool,
    pub contains_raw_data: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub contains_clinical_decision: bool,
    pub signature_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionCapsulePolicy {
    pub policy_version: String,
    pub allowed_site_order: Vec<String>,
    pub revoked_site_order: Vec<String>,
    pub allowed_action_order: Vec<String>,
    pub max_age_ticks: u64,
    pub expires_at_tick: u64,
    pub require_local_only: bool,
    pub require_no_raw_data: bool,
    pub policy_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionCapsuleRequest {
    pub question_scope: String,
    pub context: LocalDecisionContextCapsule,
    pub policy: DecisionCapsulePolicy,
    pub current_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedDecisionCapsuleDisposition {
    Accepted,
    Rejected,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionContext {
    pub feature_id: String,
    pub output_schema: String,
    pub capsule_id: String,
    pub source_site_id: String,
    pub question_scope: String,
    pub objective: String,
    pub context_digest: ContentHash,
    pub policy_digest: ContentHash,
    pub claim_order: Vec<String>,
    pub evidence_coverage_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub allowed_downstream_action_order: Vec<String>,
    pub denied_downstream_action_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub freshness_age_ticks: Option<u64>,
    pub negative_evidence_order: Vec<String>,
    pub disposition: FederatedDecisionCapsuleDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedDecisionCapsuleError {
    #[error("federated decision capsule request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated decision capsule output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated decision capsule digest failed: {0}")]
    Digest(String),
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values
            .iter()
            .all(|value| safe_text(value) && seen.insert(value.clone()))
}

fn context_body(context: &LocalDecisionContextCapsule) -> serde_json::Value {
    serde_json::json!({
        "capsule_id": context.capsule_id,
        "source_site_id": context.source_site_id,
        "objective": context.objective,
        "context_digest": context.context_digest,
        "claim_order": context.claim_order,
        "evidence_coverage_order": context.evidence_coverage_order,
        "omission_order": context.omission_order,
        "downstream_action_order": context.downstream_action_order,
        "uncertainty_order": context.uncertainty_order,
        "generated_tick": context.generated_tick,
        "expires_at_tick": context.expires_at_tick,
        "local_only": context.local_only,
        "contains_raw_data": context.contains_raw_data,
        "contains_human_data": context.contains_human_data,
        "contains_direct_identifiers": context.contains_direct_identifiers,
        "contains_clinical_decision": context.contains_clinical_decision,
    })
}

fn output_body(output: &FederatedDecisionContext) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "capsule_id": output.capsule_id,
        "source_site_id": output.source_site_id,
        "question_scope": output.question_scope,
        "objective": output.objective,
        "context_digest": output.context_digest,
        "policy_digest": output.policy_digest,
        "claim_order": output.claim_order,
        "evidence_coverage_order": output.evidence_coverage_order,
        "omission_order": output.omission_order,
        "allowed_downstream_action_order": output.allowed_downstream_action_order,
        "denied_downstream_action_order": output.denied_downstream_action_order,
        "uncertainty_order": output.uncertainty_order,
        "freshness_age_ticks": output.freshness_age_ticks,
        "negative_evidence_order": output.negative_evidence_order,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &FederatedDecisionCapsuleRequest,
) -> Result<(), FederatedDecisionCapsuleError> {
    let context = &request.context;
    let policy = &request.policy;
    if !safe_text(&request.question_scope)
        || !safe_text(&context.capsule_id)
        || !safe_text(&context.source_site_id)
        || !safe_text(&context.objective)
        || !valid_hash(&context.context_digest)
        || !unique_bounded(&context.claim_order, MAX_ITEMS)
        || !canonical(&context.claim_order)
        || !unique_bounded(&context.evidence_coverage_order, MAX_ITEMS)
        || !canonical(&context.evidence_coverage_order)
        || !unique_bounded(&context.omission_order, MAX_ITEMS)
        || !canonical(&context.omission_order)
        || !unique_bounded(&context.downstream_action_order, MAX_ITEMS)
        || !canonical(&context.downstream_action_order)
        || !unique_bounded(&context.uncertainty_order, MAX_ITEMS)
        || !canonical(&context.uncertainty_order)
        || context.generated_tick == 0
        || context.expires_at_tick < context.generated_tick
        || !valid_hash(&context.signature_digest)
        || !safe_text(&policy.policy_version)
        || !unique_bounded(&policy.allowed_site_order, MAX_ITEMS)
        || !canonical(&policy.allowed_site_order)
        || !unique_bounded(&policy.revoked_site_order, MAX_ITEMS)
        || !canonical(&policy.revoked_site_order)
        || !unique_bounded(&policy.allowed_action_order, MAX_ITEMS)
        || !canonical(&policy.allowed_action_order)
        || policy.max_age_ticks == 0
        || policy.expires_at_tick < request.current_tick
        || !valid_hash(&policy.policy_digest)
        || request.current_tick == 0
    {
        return Err(FederatedDecisionCapsuleError::InvalidRequest(
            "bounded scope, signed context identity, ordered fields, and active policy are required".into(),
        ));
    }
    if !policy
        .allowed_site_order
        .binary_search(&context.source_site_id)
        .is_ok()
    {
        return Err(FederatedDecisionCapsuleError::InvalidRequest(
            "source site is not in the allowed federation membership".into(),
        ));
    }
    Ok(())
}

impl FederatedDecisionContext {
    pub fn validate(&self) -> Result<(), FederatedDecisionCapsuleError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.capsule_id)
            || !safe_text(&self.source_site_id)
            || !safe_text(&self.question_scope)
            || !safe_text(&self.objective)
            || !valid_hash(&self.context_digest)
            || !valid_hash(&self.policy_digest)
            || !unique_bounded(&self.claim_order, MAX_ITEMS)
            || !canonical(&self.claim_order)
            || !unique_bounded(&self.evidence_coverage_order, MAX_ITEMS)
            || !canonical(&self.evidence_coverage_order)
            || !unique_bounded(&self.omission_order, MAX_ITEMS)
            || !canonical(&self.omission_order)
            || !unique_bounded(&self.allowed_downstream_action_order, MAX_ITEMS)
            || !canonical(&self.allowed_downstream_action_order)
            || !unique_bounded(&self.denied_downstream_action_order, MAX_ITEMS)
            || !canonical(&self.denied_downstream_action_order)
            || !unique_bounded(&self.uncertainty_order, MAX_ITEMS)
            || !canonical(&self.uncertainty_order)
            || !unique_bounded(&self.negative_evidence_order, MAX_ITEMS)
            || !canonical(&self.negative_evidence_order)
            || !valid_hash(&self.digest)
        {
            return Err(FederatedDecisionCapsuleError::InvalidOutput(
                "capsule identity, ordered coverage, authority partition, or digest is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| FederatedDecisionCapsuleError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedDecisionCapsuleError::InvalidOutput(
                "capsule digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Package a local context for federated downstream planning without exporting raw study data.
pub fn package_glioma_federated_decision_capsule(
    request: &FederatedDecisionCapsuleRequest,
) -> Result<FederatedDecisionContext, FederatedDecisionCapsuleError> {
    validate_request(request)?;
    let context = &request.context;
    let policy = &request.policy;
    let age = request.current_tick.saturating_sub(context.generated_tick);
    let mut negative = Vec::new();
    let mut uncertainty = context.uncertainty_order.clone();
    let mut denied = Vec::new();
    let mut allowed = Vec::new();
    let mut disposition = FederatedDecisionCapsuleDisposition::Accepted;
    if policy
        .revoked_site_order
        .binary_search(&context.source_site_id)
        .is_ok()
    {
        disposition = FederatedDecisionCapsuleDisposition::Rejected;
        negative.push("source-site-revoked".into());
    }
    if age > policy.max_age_ticks || context.expires_at_tick < request.current_tick {
        disposition = FederatedDecisionCapsuleDisposition::Rejected;
        negative.push("context-stale-or-expired".into());
    }
    if policy.expires_at_tick < request.current_tick {
        disposition = FederatedDecisionCapsuleDisposition::Rejected;
        negative.push("federation-policy-expired".into());
    }
    if (policy.require_local_only && !context.local_only)
        || (policy.require_no_raw_data
            && (context.contains_raw_data
                || context.contains_human_data
                || context.contains_direct_identifiers
                || context.contains_clinical_decision))
    {
        disposition = FederatedDecisionCapsuleDisposition::Rejected;
        negative.push("locality-or-protected-payload-violation".into());
    }
    let signature = ContentHash::of_value(&context_body(context))
        .map_err(|error| FederatedDecisionCapsuleError::Digest(error.to_string()))?;
    if signature != context.signature_digest {
        disposition = FederatedDecisionCapsuleDisposition::Rejected;
        negative.push("context-signature-invalid".into());
    }
    for action in &context.downstream_action_order {
        if policy.allowed_action_order.binary_search(action).is_ok() {
            allowed.push(action.clone());
        } else {
            denied.push(action.clone());
            negative.push(format!("action-out-of-scope:{action}"));
        }
    }
    if !denied.is_empty() {
        disposition = FederatedDecisionCapsuleDisposition::Rejected;
    }
    if context.evidence_coverage_order.is_empty() {
        disposition = FederatedDecisionCapsuleDisposition::Unresolved;
        uncertainty.push("evidence-coverage-is-empty".into());
    }
    if context.omission_order.is_empty() {
        uncertainty.push("no-explicit-omissions-were-published".into());
    }
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = FederatedDecisionContext {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        capsule_id: context.capsule_id.clone(),
        source_site_id: context.source_site_id.clone(),
        question_scope: request.question_scope.clone(),
        objective: context.objective.clone(),
        context_digest: context.context_digest.clone(),
        policy_digest: policy.policy_digest.clone(),
        claim_order: context.claim_order.clone(),
        evidence_coverage_order: context.evidence_coverage_order.clone(),
        omission_order: context.omission_order.clone(),
        allowed_downstream_action_order: allowed,
        denied_downstream_action_order: denied,
        uncertainty_order: uncertainty,
        freshness_age_ticks: Some(age),
        negative_evidence_order: negative,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-decision-capsule"),
    };
    output.digest = ContentHash::of_value(&output_body(&output))
        .map_err(|error| FederatedDecisionCapsuleError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn context() -> LocalDecisionContextCapsule {
        let mut context = LocalDecisionContextCapsule {
            capsule_id: "capsule-1".into(),
            source_site_id: "site-a".into(),
            objective: "prioritize glioma mechanism validation".into(),
            context_digest: hash("context"),
            claim_order: vec!["claim-a".into()],
            evidence_coverage_order: vec!["coverage-a".into()],
            omission_order: vec!["missing-spatial-coverage".into()],
            downstream_action_order: vec!["action-a".into()],
            uncertainty_order: vec!["uncertainty-a".into()],
            generated_tick: 1,
            expires_at_tick: 100,
            local_only: true,
            contains_raw_data: false,
            contains_human_data: false,
            contains_direct_identifiers: false,
            contains_clinical_decision: false,
            signature_digest: hash("unsealed"),
        };
        context.signature_digest =
            ContentHash::of_value(&context_body(&context)).expect("signature");
        context
    }

    fn request(context: LocalDecisionContextCapsule) -> FederatedDecisionCapsuleRequest {
        FederatedDecisionCapsuleRequest {
            question_scope: "preclinical organoid invasion".into(),
            context,
            policy: DecisionCapsulePolicy {
                policy_version: "policy-1".into(),
                allowed_site_order: vec!["site-a".into()],
                revoked_site_order: Vec::new(),
                allowed_action_order: vec!["action-a".into()],
                max_age_ticks: 10,
                expires_at_tick: 100,
                require_local_only: true,
                require_no_raw_data: true,
                policy_digest: hash("policy"),
            },
            current_tick: 5,
        }
    }

    #[test]
    fn signed_capsule_preserves_omissions_and_allowed_action_scope() {
        let output =
            package_glioma_federated_decision_capsule(&request(context())).expect("capsule");
        assert_eq!(
            output.disposition,
            FederatedDecisionCapsuleDisposition::Accepted
        );
        assert_eq!(output.omission_order, vec!["missing-spatial-coverage"]);
        assert_eq!(output.allowed_downstream_action_order, vec!["action-a"]);
    }

    #[test]
    fn stale_capsule_is_rejected_without_authority() {
        let mut context = context();
        context.generated_tick = 1;
        let mut request = request(context);
        request.current_tick = 20;
        request.policy.max_age_ticks = 5;
        let output = package_glioma_federated_decision_capsule(&request).expect("capsule");
        assert_eq!(
            output.disposition,
            FederatedDecisionCapsuleDisposition::Rejected
        );
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("stale")));
    }

    #[test]
    fn revoked_site_is_rejected() {
        let mut request = request(context());
        request.policy.revoked_site_order = vec!["site-a".into()];
        let output = package_glioma_federated_decision_capsule(&request).expect("capsule");
        assert_eq!(
            output.disposition,
            FederatedDecisionCapsuleDisposition::Rejected
        );
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("revoked")));
    }

    #[test]
    fn raw_or_clinical_payload_is_rejected() {
        let mut context = context();
        context.contains_raw_data = true;
        let request = request(context);
        let output = package_glioma_federated_decision_capsule(&request).expect("capsule");
        assert_eq!(
            output.disposition,
            FederatedDecisionCapsuleDisposition::Rejected
        );
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("protected")));
    }

    #[test]
    fn out_of_scope_action_is_rejected_and_recorded() {
        let mut context = context();
        context.downstream_action_order = vec!["action-a".into(), "action-z".into()];
        context.signature_digest =
            ContentHash::of_value(&context_body(&context)).expect("signature");
        let output = package_glioma_federated_decision_capsule(&request(context)).expect("capsule");
        assert_eq!(
            output.disposition,
            FederatedDecisionCapsuleDisposition::Rejected
        );
        assert_eq!(output.denied_downstream_action_order, vec!["action-z"]);
    }
}
